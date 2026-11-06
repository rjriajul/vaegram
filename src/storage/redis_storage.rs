use std::collections::HashMap;
use std::fmt;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use redis::aio::{ConnectionManager, ConnectionManagerConfig};
use redis::{Client, Pipeline, RedisError, cmd, pipe};

use super::engine::{PeerRecord, Reports, Storage, StorageFuture, StorageReporter, StoredState};
use super::{SessionData, UpdateState};
use crate::session::Salt;
use crate::utils::{CHANNEL_OFFSET, get_channel_id};

const VERSION: i64 = 1;
const TIMEOUT: Duration = Duration::from_secs(10);

type Hash = HashMap<String, Vec<u8>>;

/// Keeps everything in Redis, in the key layout the Python MTProto libraries use, under `vaegram:<name>` unless
/// [`prefix`](Self::prefix) says otherwise. Point it at their prefix to pick up a session they wrote.
///
/// An evicted session key is a lost login: run the database with `maxmemory-policy noeviction`, or give the client
/// a database of its own. Opening warns when the server reports another policy.
///
/// Every write is one MULTI/EXEC transaction over keys that all start with the prefix. On Redis Cluster those keys
/// must share a hash slot: give a prefix with a hash tag, such as `{vaegram:bot}`.
///
/// Each username and number also has a `<key>:claims` sorted set of every peer listing it, scored by when it was
/// last updated, so a key passes to the newest remaining holder when its owner lets it go.
///
/// Usernames are matched in lowercase, as written by this library and the Python ones.
pub struct RedisStorage {
    client: Client,
    prefix: String,
    conn: Mutex<Option<ConnectionManager>>,
    reports: Reports,
}

impl RedisStorage {
    /// `url` is a `redis://` URL; nothing connects until the storage is opened.
    pub fn new(url: &str, name: &str) -> io::Result<RedisStorage> {
        let client = Client::open(url).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "not a Redis URL"))?;
        Ok(RedisStorage {
            client,
            prefix: format!("vaegram:{name}"),
            conn: Mutex::new(None),
            reports: Reports::default(),
        })
    }

    /// Puts every key under `prefix` instead of `vaegram:<name>`.
    #[must_use]
    pub fn prefix(mut self, prefix: impl Into<String>) -> RedisStorage {
        self.prefix = prefix.into();
        self
    }

    fn key(&self, rest: impl fmt::Display) -> String {
        format!("{}:{rest}", self.prefix)
    }

    fn peer_key(&self, id: i64) -> String {
        self.key(format_args!("peer:{id}"))
    }

    fn usernames_key(&self, id: i64) -> String {
        self.key(format_args!("peer:{id}:usernames"))
    }

    async fn conn(&self) -> io::Result<ConnectionManager> {
        if let Some(conn) = self.conn.lock().unwrap_or_else(PoisonError::into_inner).clone() {
            return Ok(conn);
        }
        let config = ConnectionManagerConfig::new()
            .set_connection_timeout(Some(TIMEOUT))
            .set_response_timeout(Some(TIMEOUT))
            .set_number_of_retries(1);
        let mut conn = ConnectionManager::new_with_config(self.client.clone(), config).await.map_err(io_error)?;
        let policy: Result<Vec<String>, _> =
            cmd("CONFIG").arg("GET").arg("maxmemory-policy").query_async(&mut conn).await;
        if let Some(warning) = policy.ok().and_then(|reply| reply.get(1).and_then(|p| eviction_warning(p))) {
            self.reports.report(io::Error::other(warning));
        }
        let version: Option<i64> =
            cmd("GET").arg(self.key("version")).query_async(&mut conn).await.map_err(io_error)?;
        if version.is_some_and(|v| v > VERSION) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "this Redis session needs a newer vaegram"));
        }
        *self.conn.lock().unwrap_or_else(PoisonError::into_inner) = Some(conn.clone());
        Ok(conn)
    }

    async fn peers(&self, conn: &mut ConnectionManager, ids: &[i64]) -> io::Result<Vec<PeerRecord>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut hashes = pipe();
        let mut sets = pipe();
        for &id in ids {
            hashes.cmd("HGETALL").arg(self.peer_key(id));
            sets.cmd("SMEMBERS").arg(self.usernames_key(id));
        }
        let hashes: Vec<Hash> = hashes.query_async(conn).await.map_err(io_error)?;
        let sets: Vec<Vec<String>> = sets.query_async(conn).await.map_err(io_error)?;
        Ok(ids.iter().zip(hashes).zip(sets).filter_map(|((&id, hash), set)| record(id, &hash, set)).collect())
    }

    async fn peer(&self, conn: &mut ConnectionManager, id: i64) -> io::Result<Option<PeerRecord>> {
        Ok(self.peers(conn, &[id]).await?.pop())
    }

    /// The newest peer claiming `key`, or, for a key written without claims, the one it names.
    async fn peer_at(&self, key: String) -> io::Result<Option<PeerRecord>> {
        let mut conn = self.conn().await?;
        let newest: Vec<String> = cmd("ZREVRANGE")
            .arg(format!("{key}:claims"))
            .arg(0)
            .arg(0)
            .query_async(&mut conn)
            .await
            .map_err(io_error)?;
        let id = match newest.first().and_then(|m| decode_member(m)) {
            Some(id) => Some(id.to_string()),
            None => cmd("GET").arg(key).query_async(&mut conn).await.map_err(io_error)?,
        };
        match id.and_then(|id| id.parse().ok()) {
            Some(id) => self.peer(&mut conn, id).await,
            None => Ok(None),
        }
    }

    async fn members(&self, conn: &mut ConnectionManager, key: String) -> io::Result<Vec<String>> {
        cmd("SMEMBERS").arg(key).query_async(conn).await.map_err(io_error)
    }

    async fn load_session(&self) -> io::Result<Option<SessionData>> {
        let mut conn = self.conn().await?;
        let hash: Hash = cmd("HGETALL").arg(self.key("session")).query_async(&mut conn).await.map_err(io_error)?;
        Ok(session(&hash))
    }

    async fn save_session(&self, s: &SessionData) -> io::Result<()> {
        let mut conn = self.conn().await?;
        let (address, port) = match s.addr {
            Some(addr) => (addr.ip().to_string(), addr.port().to_string()),
            None => (String::new(), String::new()),
        };
        let mut p = pipe();
        p.atomic();
        p.cmd("HSET")
            .arg(self.key("session"))
            .arg("dc_id")
            .arg(s.dc_id)
            .arg("server_address")
            .arg(address)
            .arg("port")
            .arg(port)
            .arg("api_id")
            .arg(s.api_id)
            .arg("test_mode")
            .arg(u8::from(s.test_mode))
            .arg("auth_key")
            .arg(&s.auth_key[..])
            .arg("date")
            .arg(now())
            .arg("user_id")
            .arg(s.user_id)
            .arg("is_bot")
            .arg(u8::from(s.is_bot));
        p.cmd("SET").arg(self.key("version")).arg(VERSION);
        p.exec_async(&mut conn).await.map_err(io_error)
    }

    async fn load_state(&self) -> io::Result<Option<StoredState>> {
        let mut conn = self.conn().await?;
        let saved: Hash = cmd("HGETALL").arg(self.key("vaegram")).query_async(&mut conn).await.map_err(io_error)?;
        let ids: Vec<i64> =
            self.members(&mut conn, self.key("states")).await?.iter().filter_map(|id| id.parse().ok()).collect();
        let mut rows: Vec<Hash> = Vec::new();
        if !ids.is_empty() {
            let mut p = pipe();
            for id in &ids {
                p.cmd("HGETALL").arg(self.key(format_args!("state:{id}")));
            }
            rows = p.query_async(&mut conn).await.map_err(io_error)?;
        }
        let mut update: Option<UpdateState> = None;
        let mut channels = HashMap::new();
        for (id, row) in ids.into_iter().zip(rows) {
            let n = |field| num::<i32>(&row, field).unwrap_or(0);
            if id == 0 {
                update = Some(UpdateState {
                    pts: n("pts"),
                    qts: n("qts"),
                    date: n("date"),
                    seq: n("seq"),
                    ..UpdateState::default()
                });
            } else if id <= -CHANNEL_OFFSET {
                channels.insert(get_channel_id(id), n("pts"));
            }
        }
        if let Some(update) = &mut update {
            update.channels = channels;
        }
        let update = update.filter(|u| u.date != 0);
        if let Some(owner) = num(&saved, "owner") {
            let salt =
                Salt { value: num(&saved, "salt").unwrap_or(0), time_offset: num(&saved, "time_offset").unwrap_or(0) };
            return Ok(Some(StoredState { owner, salt, update }));
        }
        if update.is_none() {
            return Ok(None);
        }
        // Another library's session: its account owns the update state.
        let owner = self.load_session().await?.map_or(0, |s| s.user_id);
        Ok(Some(StoredState { owner, salt: Salt::default(), update }))
    }

    async fn save_state(&self, state: &StoredState) -> io::Result<()> {
        let mut conn = self.conn().await?;
        let known = self.members(&mut conn, self.key("states")).await?;
        let mut p = pipe();
        p.atomic();
        p.cmd("HSET")
            .arg(self.key("vaegram"))
            .arg("owner")
            .arg(state.owner)
            .arg("salt")
            .arg(state.salt.value)
            .arg("time_offset")
            .arg(state.salt.time_offset);
        for id in &known {
            p.cmd("DEL").arg(self.key(format_args!("state:{id}")));
        }
        p.cmd("DEL").arg(self.key("states"));
        if let Some(u) = state.update.as_ref().filter(|u| u.date != 0) {
            p.cmd("HSET")
                .arg(self.key("state:0"))
                .arg("pts")
                .arg(u.pts)
                .arg("qts")
                .arg(u.qts)
                .arg("date")
                .arg(u.date)
                .arg("seq")
                .arg(u.seq);
            p.cmd("SADD").arg(self.key("states")).arg(0);
            for (&channel, &pts) in &u.channels {
                let id = get_channel_id(channel);
                p.cmd("HSET").arg(self.key(format_args!("state:{id}"))).arg("pts").arg(pts);
                p.cmd("SADD").arg(self.key("states")).arg(id);
            }
        }
        p.exec_async(&mut conn).await.map_err(io_error)
    }

    async fn update_peers(&self, peers: &[PeerRecord]) -> io::Result<()> {
        // The last record of each peer in the batch is the one kept.
        let mut batch: HashMap<i64, &PeerRecord> = HashMap::new();
        for p in peers {
            batch.insert(p.id, p);
        }
        if batch.is_empty() {
            return Ok(());
        }
        let ids: Vec<i64> = batch.keys().copied().collect();
        let mut conn = self.conn().await?;
        let (mut phones, mut sets) = (pipe(), pipe());
        for &id in &ids {
            phones.cmd("HGET").arg(self.peer_key(id)).arg("phone_number");
            sets.cmd("SMEMBERS").arg(self.usernames_key(id));
        }
        let old_phones: Vec<Option<String>> = phones.query_async(&mut conn).await.map_err(io_error)?;
        let old_names: Vec<Vec<String>> = sets.query_async(&mut conn).await.map_err(io_error)?;

        // Every username and number key the batch holds now or held before, with its claims as stored.
        let mut keys: Vec<String> = Vec::new();
        for (i, &id) in ids.iter().enumerate() {
            let p = batch[&id];
            keys.extend(old_names[i].iter().chain(&p.usernames).map(|u| format!("username:{u}")));
            let phones = old_phones[i].iter().filter(|n| !n.is_empty()).chain(&p.phone_number);
            keys.extend(phones.map(|n| format!("phone:{n}")));
        }
        keys.sort_unstable();
        keys.dedup();
        let mut claims: Vec<HashMap<i64, i64>> = Vec::new();
        let mut owners: Vec<Option<String>> = Vec::new();
        if !keys.is_empty() {
            let (mut stored, mut held) = (pipe(), pipe());
            for key in &keys {
                stored.cmd("ZRANGE").arg(self.key(format_args!("{key}:claims"))).arg(0).arg(-1).arg("WITHSCORES");
                held.cmd("GET").arg(self.key(key));
            }
            let stored: Vec<Vec<String>> = stored.query_async(&mut conn).await.map_err(io_error)?;
            claims = stored.iter().map(|flat| decode_claims(flat)).collect();
            owners = held.query_async(&mut conn).await.map_err(io_error)?;
        }

        // A key written without claims, by another library, keeps its holder as one.
        let unclaimed: Vec<(usize, i64)> = owners
            .iter()
            .enumerate()
            .filter_map(|(i, owner)| Some((i, owner.as_deref()?.parse().ok()?)))
            .filter(|&(i, id)| claims[i].is_empty() && !batch.contains_key(&id))
            .collect();
        if !unclaimed.is_empty() {
            let mut p = pipe();
            for &(_, id) in &unclaimed {
                p.cmd("HGET").arg(self.peer_key(id)).arg("last_update_on");
            }
            let updated: Vec<Option<i64>> = p.query_async(&mut conn).await.map_err(io_error)?;
            for (&(i, id), updated) in unclaimed.iter().zip(updated) {
                claims[i].insert(id, updated.unwrap_or(0));
            }
        }
        for (key, claim) in keys.iter().zip(&mut claims) {
            for &id in &ids {
                claim.remove(&id);
            }
            for p in batch.values() {
                let holds = match key.split_once(':') {
                    Some(("username", name)) => p.usernames.iter().any(|u| u == name),
                    Some((_, number)) => p.phone_number.as_deref() == Some(number),
                    None => false,
                };
                if holds {
                    claim.insert(p.id, p.updated);
                }
            }
        }

        let mut writes = pipe();
        writes.atomic();
        for &id in &ids {
            let p = batch[&id];
            writes
                .cmd("HSET")
                .arg(self.peer_key(id))
                .arg("access_hash")
                .arg(p.access_hash)
                .arg("type")
                .arg(peer_type(id))
                .arg("phone_number")
                .arg(p.phone_number.as_deref().unwrap_or(""))
                .arg("last_update_on")
                .arg(p.updated)
                .arg("usernames")
                .arg(p.usernames.join(","));
            writes.cmd("SADD").arg(self.key("peers")).arg(id);
            writes.cmd("DEL").arg(self.usernames_key(id));
            if !p.usernames.is_empty() {
                writes.cmd("SADD").arg(self.usernames_key(id)).arg(&p.usernames);
            }
        }
        for ((key, claim), owner) in keys.iter().zip(&claims).zip(&owners) {
            let claims_key = self.key(format_args!("{key}:claims"));
            writes.cmd("DEL").arg(&claims_key);
            for (&id, &updated) in claim {
                writes.cmd("ZADD").arg(&claims_key).arg(updated).arg(member(id));
            }
            // The key other libraries read: the newest claim, the larger id on a tie.
            match claim.iter().map(|(&id, &updated)| (updated, id)).max() {
                Some((_, winner)) if owner.as_deref() != Some(winner.to_string().as_str()) => {
                    writes.cmd("SET").arg(self.key(key)).arg(winner)
                }
                Some(_) => continue,
                None => writes.cmd("DEL").arg(self.key(key)),
            };
        }
        writes.exec_async(&mut conn).await.map_err(io_error)
    }

    async fn export_peers(&self, limit: Option<usize>) -> io::Result<Vec<PeerRecord>> {
        let mut conn = self.conn().await?;
        let ids: Vec<i64> =
            self.members(&mut conn, self.key("peers")).await?.iter().filter_map(|id| id.parse().ok()).collect();
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut p = pipe();
        for &id in &ids {
            p.cmd("HGET").arg(self.peer_key(id)).arg("last_update_on");
        }
        let updated: Vec<Option<i64>> = p.query_async(&mut conn).await.map_err(io_error)?;
        let mut newest: Vec<(i64, i64)> = ids.into_iter().zip(updated).map(|(id, u)| (u.unwrap_or(0), id)).collect();
        newest.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        let ids: Vec<i64> = newest.into_iter().take(limit.unwrap_or(usize::MAX)).map(|(_, id)| id).collect();
        self.peers(&mut conn, &ids).await
    }

    async fn clear_peers(&self) -> io::Result<()> {
        let mut conn = self.conn().await?;
        let mut writes = pipe();
        writes.atomic();
        self.clear_peers_into(&mut conn, &mut writes).await?;
        writes.exec_async(&mut conn).await.map_err(io_error)
    }

    /// Adds to `writes` the deletes that forget every peer.
    async fn clear_peers_into(&self, conn: &mut ConnectionManager, writes: &mut Pipeline) -> io::Result<()> {
        let ids: Vec<i64> =
            self.members(conn, self.key("peers")).await?.iter().filter_map(|id| id.parse().ok()).collect();
        if !ids.is_empty() {
            let (mut phones, mut sets) = (pipe(), pipe());
            for &id in &ids {
                phones.cmd("HGET").arg(self.peer_key(id)).arg("phone_number");
                sets.cmd("SMEMBERS").arg(self.usernames_key(id));
            }
            let phones: Vec<Option<String>> = phones.query_async(conn).await.map_err(io_error)?;
            let sets: Vec<Vec<String>> = sets.query_async(conn).await.map_err(io_error)?;
            for ((id, phone), names) in ids.iter().zip(phones).zip(sets) {
                for name in names {
                    writes.cmd("DEL").arg(self.key(format_args!("username:{name}")));
                    writes.cmd("DEL").arg(self.key(format_args!("username:{name}:claims")));
                }
                if let Some(phone) = phone.filter(|p| !p.is_empty()) {
                    writes.cmd("DEL").arg(self.key(format_args!("phone:{phone}")));
                    writes.cmd("DEL").arg(self.key(format_args!("phone:{phone}:claims")));
                }
                writes.cmd("DEL").arg(self.usernames_key(*id)).arg(self.peer_key(*id));
            }
        }
        writes.cmd("DEL").arg(self.key("peers"));
        Ok(())
    }

    async fn delete(&self) -> io::Result<()> {
        let mut conn = self.conn().await?;
        let mut p = pipe();
        p.atomic();
        self.clear_peers_into(&mut conn, &mut p).await?;
        for id in self.members(&mut conn, self.key("states")).await? {
            p.cmd("DEL").arg(self.key(format_args!("state:{id}")));
        }
        for key in ["states", "session", "version", "vaegram"] {
            p.cmd("DEL").arg(self.key(key));
        }
        p.exec_async(&mut conn).await.map_err(io_error)
    }
}

fn io_error(e: RedisError) -> io::Error {
    let kind = if e.is_timeout() {
        io::ErrorKind::TimedOut
    } else if e.is_io_error() || e.is_connection_dropped() {
        io::ErrorKind::ConnectionAborted
    } else {
        io::ErrorKind::Other
    };
    io::Error::new(kind, format!("redis: {e}"))
}

fn eviction_warning(policy: &str) -> Option<String> {
    (policy != "noeviction").then(|| {
        format!(
            "Redis evicts keys under maxmemory-policy {policy}, and an evicted session key is a lost login: \
             use noeviction, or a database of its own"
        )
    })
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// A claim set member: the peer id as 16 hex digits with the sign bit flipped, so members sort as the ids do and
/// ties between equal scores go to the larger id.
fn member(id: i64) -> String {
    format!("{:016x}", (id as u64) ^ (1 << 63))
}

fn decode_member(member: &str) -> Option<i64> {
    u64::from_str_radix(member, 16).ok().map(|bits| (bits ^ (1 << 63)) as i64)
}

/// `ZRANGE … WITHSCORES` as peer id → updated.
fn decode_claims(flat: &[String]) -> HashMap<i64, i64> {
    flat.chunks(2)
        .filter_map(|pair| Some((decode_member(pair.first()?)?, pair.get(1)?.parse::<f64>().ok()? as i64)))
        .collect()
}

fn peer_type(id: i64) -> &'static str {
    if id > 0 {
        "user"
    } else if id <= -CHANNEL_OFFSET {
        "channel"
    } else {
        "group"
    }
}

fn text<'a>(hash: &'a Hash, field: &str) -> Option<&'a str> {
    hash.get(field).and_then(|v| std::str::from_utf8(v).ok()).filter(|s| !s.is_empty())
}

fn num<T: FromStr>(hash: &Hash, field: &str) -> Option<T> {
    text(hash, field)?.parse().ok()
}

fn session(hash: &Hash) -> Option<SessionData> {
    let auth_key = hash.get("auth_key").and_then(|k| <[u8; 256]>::try_from(k.as_slice()).ok())?;
    let mut s = SessionData::new(num(hash, "dc_id")?, auth_key);
    let ip = text(hash, "server_address").and_then(|a| a.parse::<IpAddr>().ok());
    s.addr = ip.zip(num::<u16>(hash, "port").filter(|&p| p != 0)).map(|(ip, port)| SocketAddr::new(ip, port));
    s.api_id = num(hash, "api_id").unwrap_or(0);
    s.test_mode = num::<i64>(hash, "test_mode").is_some_and(|v| v != 0);
    s.user_id = num(hash, "user_id").unwrap_or(0);
    s.is_bot = num::<i64>(hash, "is_bot").is_some_and(|v| v != 0);
    Some(s)
}

// Peers written elsewhere carry their usernames only in the set, in no order; ours keep the order in the hash.
fn record(id: i64, hash: &Hash, mut set: Vec<String>) -> Option<PeerRecord> {
    if hash.is_empty() {
        return None;
    }
    let mut p = PeerRecord::new(id, num(hash, "access_hash").unwrap_or(0));
    p.phone_number = text(hash, "phone_number").map(str::to_owned);
    p.updated = num(hash, "last_update_on").unwrap_or(0);
    p.usernames = match hash.get("usernames").map(|v| String::from_utf8_lossy(v)) {
        Some(ordered) => ordered.split(',').filter(|u| !u.is_empty()).map(str::to_owned).collect(),
        None => {
            set.sort_unstable();
            set
        }
    };
    Some(p)
}

impl fmt::Debug for RedisStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedisStorage").field("prefix", &self.prefix).finish_non_exhaustive()
    }
}

impl Storage for RedisStorage {
    fn set_reporter(&self, reporter: StorageReporter) {
        self.reports.set(reporter);
    }

    fn open(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move { self.conn().await.map(drop) })
    }

    fn close(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            self.conn.lock().unwrap_or_else(PoisonError::into_inner).take();
            Ok(())
        })
    }

    fn delete(&self) -> StorageFuture<'_, ()> {
        Box::pin(self.delete())
    }

    fn load_session(&self) -> StorageFuture<'_, Option<SessionData>> {
        Box::pin(self.load_session())
    }

    fn save_session<'a>(&'a self, session: &'a SessionData) -> StorageFuture<'a, ()> {
        Box::pin(self.save_session(session))
    }

    fn load_state(&self) -> StorageFuture<'_, Option<StoredState>> {
        Box::pin(self.load_state())
    }

    fn save_state<'a>(&'a self, state: &'a StoredState) -> StorageFuture<'a, ()> {
        Box::pin(self.save_state(state))
    }

    fn update_peers<'a>(&'a self, peers: &'a [PeerRecord]) -> StorageFuture<'a, ()> {
        Box::pin(self.update_peers(peers))
    }

    fn get_peer_by_id(&self, id: i64) -> StorageFuture<'_, Option<PeerRecord>> {
        Box::pin(async move {
            let mut conn = self.conn().await?;
            self.peer(&mut conn, id).await
        })
    }

    fn get_peer_by_username<'a>(&'a self, username: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        Box::pin(self.peer_at(self.key(format_args!("username:{username}"))))
    }

    fn get_peer_by_phone_number<'a>(&'a self, phone_number: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        Box::pin(self.peer_at(self.key(format_args!("phone:{phone_number}"))))
    }

    fn export_peers(&self, limit: Option<usize>) -> StorageFuture<'_, Vec<PeerRecord>> {
        Box::pin(self.export_peers(limit))
    }

    fn clear_peers(&self) -> StorageFuture<'_, ()> {
        Box::pin(self.clear_peers())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashMap};
    use std::net::SocketAddr;
    use std::sync::{Arc, Mutex};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::task::AbortHandle;

    use super::{RedisStorage, eviction_warning};
    use crate::session::Salt;
    use crate::storage::{PeerRecord, SessionData, Storage, StoredState, UpdateState, conformance};

    enum Entry {
        Text(Vec<u8>),
        Hash(Vec<(Vec<u8>, Vec<u8>)>),
        Set(BTreeSet<Vec<u8>>),
        // Members by (score, member), as Redis orders them.
        Sorted(BTreeSet<(i64, Vec<u8>)>),
    }

    #[derive(Default)]
    struct Db {
        keys: HashMap<Vec<u8>, Entry>,
        policy: String,
        // Drop the connection at EXEC without running what was queued, as a server lost mid-write would.
        fail_exec: bool,
    }

    /// Just enough of a Redis server for the commands the storage sends.
    struct MiniRedis {
        addr: SocketAddr,
        db: Arc<Mutex<Db>>,
        connections: Arc<Mutex<Vec<AbortHandle>>>,
    }

    impl MiniRedis {
        async fn start(policy: &str) -> MiniRedis {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let db = Arc::new(Mutex::new(Db { policy: policy.into(), ..Db::default() }));
            let connections = Arc::new(Mutex::new(Vec::new()));
            let (shared, tasks) = (db.clone(), connections.clone());
            tokio::spawn(async move {
                while let Ok((stream, _)) = listener.accept().await {
                    let task = tokio::spawn(serve(stream, shared.clone()));
                    tasks.lock().unwrap().push(task.abort_handle());
                }
            });
            MiniRedis { addr, db, connections }
        }

        fn url(&self) -> String {
            format!("redis://{}", self.addr)
        }

        fn drop_connections(&self) {
            for task in self.connections.lock().unwrap().drain(..) {
                task.abort();
            }
        }

        fn text(&self, key: &str) -> Option<String> {
            match self.db.lock().unwrap().keys.get(key.as_bytes()) {
                Some(Entry::Text(v)) => Some(String::from_utf8_lossy(v).into_owned()),
                _ => None,
            }
        }

        fn field(&self, key: &str, field: &str) -> Option<String> {
            match self.db.lock().unwrap().keys.get(key.as_bytes()) {
                Some(Entry::Hash(h)) => {
                    h.iter().find(|(f, _)| f == field.as_bytes()).map(|(_, v)| String::from_utf8_lossy(v).into_owned())
                }
                _ => None,
            }
        }

        fn members(&self, key: &str) -> Vec<String> {
            match self.db.lock().unwrap().keys.get(key.as_bytes()) {
                Some(Entry::Set(s)) => s.iter().map(|m| String::from_utf8_lossy(m).into_owned()).collect(),
                _ => Vec::new(),
            }
        }

        fn run(&self, args: &[&str]) {
            let args: Vec<Vec<u8>> = args.iter().map(|a| a.as_bytes().to_vec()).collect();
            execute(&mut self.db.lock().unwrap(), &args);
        }
    }

    async fn serve(mut stream: TcpStream, db: Arc<Mutex<Db>>) {
        let mut buf = Vec::new();
        let mut chunk = [0; 4096];
        let mut queued: Option<Vec<Vec<Vec<u8>>>> = None;
        loop {
            while let Some((args, used)) = parse(&buf) {
                buf.drain(..used);
                let name = String::from_utf8_lossy(&args[0]).to_ascii_uppercase();
                let reply = match (name.as_str(), queued.is_some()) {
                    ("MULTI", _) => {
                        queued = Some(Vec::new());
                        b"+OK\r\n".to_vec()
                    }
                    ("EXEC", true) => {
                        let mut db = db.lock().unwrap();
                        if db.fail_exec {
                            return;
                        }
                        let commands = queued.take().unwrap_or_default();
                        array(commands.iter().map(|c| execute(&mut db, c)).collect::<Vec<_>>().into_iter())
                    }
                    ("DISCARD", true) => {
                        queued = None;
                        b"+OK\r\n".to_vec()
                    }
                    (_, true) => {
                        queued.get_or_insert_with(Vec::new).push(args);
                        b"+QUEUED\r\n".to_vec()
                    }
                    (_, false) => execute(&mut db.lock().unwrap(), &args),
                };
                if stream.write_all(&reply).await.is_err() {
                    return;
                }
            }
            match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => return,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
            }
        }
    }

    fn line(buf: &[u8]) -> Option<(&[u8], &[u8])> {
        let end = buf.windows(2).position(|w| w == b"\r\n")?;
        Some((&buf[..end], &buf[end + 2..]))
    }

    fn parse(buf: &[u8]) -> Option<(Vec<Vec<u8>>, usize)> {
        let (head, mut rest) = line(buf.strip_prefix(b"*")?)?;
        let count: usize = std::str::from_utf8(head).ok()?.parse().ok()?;
        let mut args = Vec::with_capacity(count);
        for _ in 0..count {
            let (len, tail) = line(rest.strip_prefix(b"$")?)?;
            let len: usize = std::str::from_utf8(len).ok()?.parse().ok()?;
            args.push(tail.get(..len)?.to_vec());
            rest = tail.get(len + 2..)?;
        }
        Some((args, buf.len() - rest.len()))
    }

    fn bulk(v: &[u8]) -> Vec<u8> {
        [format!("${}\r\n", v.len()).as_bytes(), v, b"\r\n"].concat()
    }

    fn array(items: impl ExactSizeIterator<Item = Vec<u8>>) -> Vec<u8> {
        let mut out = format!("*{}\r\n", items.len()).into_bytes();
        items.for_each(|i| out.extend(i));
        out
    }

    fn execute(db: &mut Db, args: &[Vec<u8>]) -> Vec<u8> {
        let name = String::from_utf8_lossy(&args[0]).to_ascii_uppercase();
        let int = |n: usize| format!(":{n}\r\n").into_bytes();
        let wrong = b"-WRONGTYPE wrong kind of value\r\n".to_vec();
        match (name.as_str(), &args[1..]) {
            ("PING", _) => b"+PONG\r\n".to_vec(),
            ("CONFIG", [_, name]) => array([bulk(name), bulk(db.policy.as_bytes())].into_iter()),
            ("GET", [key]) => match db.keys.get(key) {
                Some(Entry::Text(v)) => bulk(v),
                None => b"$-1\r\n".to_vec(),
                Some(_) => wrong,
            },
            ("SET", [key, value]) => {
                db.keys.insert(key.clone(), Entry::Text(value.clone()));
                b"+OK\r\n".to_vec()
            }
            ("DEL", keys) => int(keys.iter().filter(|k| db.keys.remove(*k).is_some()).count()),
            ("HSET", [key, pairs @ ..]) => {
                let entry = db.keys.entry(key.clone()).or_insert_with(|| Entry::Hash(Vec::new()));
                let Entry::Hash(hash) = entry else { return wrong };
                for pair in pairs.chunks(2) {
                    match hash.iter_mut().find(|(f, _)| *f == pair[0]) {
                        Some(slot) => slot.1 = pair[1].clone(),
                        None => hash.push((pair[0].clone(), pair[1].clone())),
                    }
                }
                int(pairs.len() / 2)
            }
            ("HGET", [key, field]) => match db.keys.get(key) {
                Some(Entry::Hash(h)) => {
                    h.iter().find(|(f, _)| f == field).map_or(b"$-1\r\n".to_vec(), |(_, v)| bulk(v))
                }
                None => b"$-1\r\n".to_vec(),
                Some(_) => wrong,
            },
            ("HGETALL", [key]) => match db.keys.get(key) {
                Some(Entry::Hash(h)) => {
                    array(h.iter().flat_map(|(f, v)| [bulk(f), bulk(v)]).collect::<Vec<_>>().into_iter())
                }
                None => b"*0\r\n".to_vec(),
                Some(_) => wrong,
            },
            ("SADD", [key, members @ ..]) => {
                let entry = db.keys.entry(key.clone()).or_insert_with(|| Entry::Set(BTreeSet::new()));
                let Entry::Set(set) = entry else { return wrong };
                int(members.iter().filter(|m| set.insert((*m).clone())).count())
            }
            ("ZADD", [key, score, member]) => {
                let entry = db.keys.entry(key.clone()).or_insert_with(|| Entry::Sorted(BTreeSet::new()));
                let Entry::Sorted(set) = entry else { return wrong };
                let score: i64 = String::from_utf8_lossy(score).parse().unwrap();
                let added = !set.iter().any(|(_, m)| m == member);
                set.retain(|(_, m)| m != member);
                set.insert((score, member.clone()));
                int(usize::from(added))
            }
            ("ZRANGE" | "ZREVRANGE", [key, start, stop, scores @ ..]) => match db.keys.get(key) {
                Some(Entry::Sorted(set)) => {
                    let mut members: Vec<&(i64, Vec<u8>)> = set.iter().collect();
                    if name == "ZREVRANGE" {
                        members.reverse();
                    }
                    let index = |v: &[u8]| {
                        let i: i64 = String::from_utf8_lossy(v).parse().unwrap();
                        if i < 0 { members.len() as i64 + i } else { i }
                    };
                    let (start, stop) = (index(start).max(0) as usize, index(stop));
                    let picked = members.iter().skip(start).take((stop + 1 - start as i64).max(0) as usize);
                    let with_scores = !scores.is_empty();
                    let items: Vec<Vec<u8>> = picked
                        .flat_map(|(score, m)| {
                            let mut item = vec![bulk(m)];
                            if with_scores {
                                item.push(bulk(score.to_string().as_bytes()));
                            }
                            item
                        })
                        .collect();
                    array(items.into_iter())
                }
                None => b"*0\r\n".to_vec(),
                Some(_) => wrong,
            },
            ("SMEMBERS", [key]) => match db.keys.get(key) {
                Some(Entry::Set(s)) => array(s.iter().map(|m| bulk(m)).collect::<Vec<_>>().into_iter()),
                None => b"*0\r\n".to_vec(),
                Some(_) => wrong,
            },
            _ => format!("-ERR unknown command '{name}'\r\n").into_bytes(),
        }
    }

    #[tokio::test]
    async fn conformance() {
        let server = MiniRedis::start("noeviction").await;
        conformance::check(&RedisStorage::new(&server.url(), "bot").unwrap()).await;
        let left: Vec<Vec<u8>> = server.db.lock().unwrap().keys.keys().cloned().collect();
        assert_eq!(left, Vec::<Vec<u8>>::new(), "delete leaves no key behind");
    }

    #[tokio::test]
    async fn key_layout_matches() {
        let server = MiniRedis::start("noeviction").await;
        let storage = RedisStorage::new(&server.url(), "ignored").unwrap().prefix("app:bot");
        let mut session = SessionData::new(2, [7; 256]);
        (session.user_id, session.is_bot, session.addr) = (42, true, Some("149.154.167.51:443".parse().unwrap()));
        storage.save_session(&session).await.unwrap();
        let mut user = PeerRecord::new(100, 5);
        (user.usernames, user.phone_number, user.updated) =
            (vec!["alice".into(), "al".into()], Some("1555".into()), 30);
        storage.update_peers(&[user, PeerRecord::new(-1_000_000_001_234, 6)]).await.unwrap();

        assert_eq!(server.field("app:bot:session", "server_address").as_deref(), Some("149.154.167.51"));
        assert_eq!(server.field("app:bot:session", "user_id").as_deref(), Some("42"));
        assert_eq!(server.field("app:bot:session", "is_bot").as_deref(), Some("1"));
        assert_eq!(server.text("app:bot:version").as_deref(), Some("1"));
        assert_eq!(server.field("app:bot:peer:100", "type").as_deref(), Some("user"));
        assert_eq!(server.field("app:bot:peer:-1000000001234", "type").as_deref(), Some("channel"));
        assert_eq!(server.field("app:bot:peer:100", "last_update_on").as_deref(), Some("30"));
        assert_eq!(server.members("app:bot:peers"), ["-1000000001234", "100"]);
        assert_eq!(server.members("app:bot:peer:100:usernames"), ["al", "alice"]);
        assert_eq!(server.text("app:bot:username:alice").as_deref(), Some("100"));
        assert_eq!(server.text("app:bot:phone:1555").as_deref(), Some("100"));
    }

    #[tokio::test]
    async fn reads_data_another_library_wrote() {
        let server = MiniRedis::start("noeviction").await;
        server.run(&["HSET", "other:bot:session", "dc_id", "4", "server_address", "", "port", "", "api_id", "1"]);
        server.run(&[
            "HSET",
            "other:bot:peer:100",
            "access_hash",
            "5",
            "type",
            "bot",
            "phone_number",
            "",
            "last_update_on",
            "9",
        ]);
        server.run(&["SADD", "other:bot:peers", "100"]);
        server.run(&["SADD", "other:bot:peer:100:usernames", "zeta", "alpha"]);
        server.run(&["SET", "other:bot:username:zeta", "100"]);
        server.run(&["HSET", "other:bot:state:0", "pts", "7", "qts", "1", "date", "1700000000", "seq", "2"]);
        server.run(&["SADD", "other:bot:states", "0"]);

        let storage = RedisStorage::new(&server.url(), "bot").unwrap().prefix("other:bot");
        let peer = storage.get_peer_by_username("zeta").await.unwrap().unwrap();
        let state = storage.load_state().await.unwrap().unwrap();
        assert_eq!(
            (peer.id, peer.access_hash, peer.usernames, peer.updated),
            (100, 5, vec!["alpha".into(), "zeta".into()], 9)
        );
        assert_eq!(peer.phone_number, None);
        assert_eq!(state.update.map(|u| (u.pts, u.date)), Some((7, 1_700_000_000)));
        assert!(storage.load_session().await.unwrap().is_none(), "a session without a key isn't a login");
    }

    #[test]
    fn eviction_warning_text() {
        assert_eq!(eviction_warning("noeviction"), None);
        let warning = eviction_warning("allkeys-lru").unwrap();
        assert!(warning.contains("allkeys-lru") && warning.contains("lost login") && warning.contains("noeviction"));
    }

    #[tokio::test]
    async fn an_evicting_server_is_reported_once_on_open() {
        let evicting = MiniRedis::start("allkeys-lru").await;
        let storage = RedisStorage::new(&evicting.url(), "bot").unwrap();
        let reports = conformance::collect_reports(&storage);
        storage.open().await.unwrap();
        storage.save_state(&StoredState::new(1, Salt::default(), None)).await.unwrap();
        let reports = reports.lock().unwrap().clone();
        assert_eq!(reports.len(), 1, "{reports:?}");
        assert!(reports[0].contains("allkeys-lru") && reports[0].contains("lost login"), "{reports:?}");

        let quiet = MiniRedis::start("noeviction").await;
        let storage = RedisStorage::new(&quiet.url(), "bot").unwrap();
        let reports = conformance::collect_reports(&storage);
        storage.open().await.unwrap();
        assert!(reports.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn debug_hides_the_password() {
        let storage = RedisStorage::new("redis://:hunter2@127.0.0.1:1/0", "bot").unwrap();
        let debug = format!("{storage:?}");
        let error = storage.open().await.unwrap_err().to_string();
        assert!(!debug.contains("hunter2") && debug.contains("vaegram:bot"), "{debug}");
        assert!(!error.contains("hunter2"), "{error}");
        assert!(RedisStorage::new("not a url", "bot").is_err());
    }

    #[tokio::test]
    async fn a_write_is_all_or_nothing() {
        let server = MiniRedis::start("noeviction").await;
        let storage = RedisStorage::new(&server.url(), "bot").unwrap();
        let update = UpdateState { pts: 1, date: 5, ..UpdateState::default() };
        let first = StoredState::new(1, Salt::default(), Some(update));
        storage.save_state(&first).await.unwrap();

        server.db.lock().unwrap().fail_exec = true;
        let update = UpdateState { pts: 2, date: 6, channels: HashMap::from([(9, 3)]), ..UpdateState::default() };
        let failed = storage.save_state(&StoredState::new(1, Salt { value: 7, time_offset: 0 }, Some(update))).await;
        server.db.lock().unwrap().fail_exec = false;
        let mut loaded = storage.load_state().await;
        if loaded.is_err() {
            loaded = storage.load_state().await;
        }
        assert!(failed.is_err());
        assert_eq!(loaded.unwrap(), Some(first), "the lost write left the state as it was");
    }

    #[tokio::test]
    #[ignore = "needs VAEGRAM_REDIS_URL, a Redis server it may write to"]
    async fn conformance_on_a_live_server() {
        let Ok(url) = std::env::var("VAEGRAM_REDIS_URL") else {
            // The CI job that runs this has a server; without the variable it would pass having tested nothing.
            assert!(std::env::var_os("CI").is_none(), "set VAEGRAM_REDIS_URL");
            return;
        };
        let prefix = format!("vaegram:live:{}", std::process::id());
        conformance::check(&RedisStorage::new(&url, "bot").unwrap().prefix(prefix)).await;
    }

    #[tokio::test]
    async fn reconnects_after_the_server_drops() {
        let server = MiniRedis::start("allkeys-lru").await;
        let storage = RedisStorage::new(&server.url(), "bot").unwrap();
        storage.save_session(&SessionData::new(2, [1; 256])).await.unwrap();
        server.drop_connections();
        let mut loaded = storage.load_session().await;
        if loaded.is_err() {
            loaded = storage.load_session().await;
        }
        assert_eq!(loaded.unwrap().map(|s| s.dc_id), Some(2));
    }
}
