use std::collections::HashMap;
use std::fmt;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio_postgres::error::SqlState;
use tokio_postgres::tls::{MakeTlsConnect, TlsConnect};
use tokio_postgres::types::FromSql;
use tokio_postgres::{Client, Config, Row, Socket};

use super::engine::{PeerRecord, Reports, Storage, StorageFuture, StorageReporter, StoredState};
use super::{SessionData, UpdateState};
use crate::session::Salt;
use crate::utils::{CHANNEL_OFFSET, get_channel_id};

const VERSION: i64 = 1;
/// How long a connection and each operation may take before the engine gives up on it.
const TIMEOUT: Duration = Duration::from_secs(10);

// The tables of `sqlite_storage.rs`, column for column, in PostgreSQL types: a session moves between the two
// engines field for field, which is what keeps the session format the same.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sessions
(
    dc_id          INTEGER PRIMARY KEY,
    server_address TEXT,
    port           INTEGER,
    api_id         INTEGER,
    test_mode      BOOLEAN,
    auth_key       BYTEA,
    date           BIGINT NOT NULL,
    user_id        BIGINT,
    is_bot         BOOLEAN
);

CREATE TABLE IF NOT EXISTS peers
(
    id             BIGINT PRIMARY KEY,
    access_hash    BIGINT,
    type           TEXT NOT NULL,
    phone_number   TEXT,
    last_update_on BIGINT NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS usernames
(
    id       BIGINT NOT NULL REFERENCES peers (id) ON DELETE CASCADE,
    username TEXT NOT NULL,
    position INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS update_state
(
    id   BIGINT PRIMARY KEY,
    pts  INTEGER,
    qts  INTEGER,
    date INTEGER,
    seq  INTEGER
);

CREATE TABLE IF NOT EXISTS version
(
    number BIGINT PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS vaegram
(
    id          INTEGER PRIMARY KEY CHECK (id = 0),
    owner       BIGINT NOT NULL,
    salt        BIGINT NOT NULL,
    time_offset BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_peers_phone_number ON peers (phone_number);
CREATE INDEX IF NOT EXISTS idx_peers_last_update_on ON peers (last_update_on DESC);
CREATE INDEX IF NOT EXISTS idx_usernames_id ON usernames (id);
CREATE INDEX IF NOT EXISTS idx_usernames_username ON usernames (username);
";

const DELETE_ALL: &str = "DELETE FROM usernames; DELETE FROM peers; DELETE FROM update_state; \
     DELETE FROM sessions; DELETE FROM vaegram; DELETE FROM version;";

const UPSERT_PEER: &str = "INSERT INTO peers (id, access_hash, type, phone_number, last_update_on) \
     VALUES ($1, $2, $3, $4, $5) \
     ON CONFLICT (id) DO UPDATE SET access_hash = EXCLUDED.access_hash, type = EXCLUDED.type, \
     phone_number = EXCLUDED.phone_number, last_update_on = EXCLUDED.last_update_on";

const NEWER: &str = "this PostgreSQL session needs a newer vaegram";

/// Keeps everything in a PostgreSQL database, in a schema of its own named `name`, in tables that mirror the
/// session layout of the SQLite engine (`sqlite_storage.rs`): the same fields, so a session moves between the two.
///
/// One schema per session: two clients on one database keep their own logins, and the schema is created when the
/// engine opens if it isn't there, so the role needs `CREATE` on the database, or a schema it owns already.
///
/// The constructor only holds the [`Config`], the TLS connector and the schema name. Connecting, spawning the
/// connection's own task and reconnecting all happen when the client opens the engine and on every call after, on
/// the store thread's runtime, so the caller's runtime is never blocked on and a dropped client still saves.
///
/// A connection the server ended is left behind and the next call opens a new one. Usernames are matched in
/// lowercase, as written by this library and the Python MTProto libraries.
pub struct PostgresStorage<T> {
    config: Config,
    tls: T,
    name: String,
    timeout: Duration,
    conn: Mutex<Option<Client>>,
    reports: Arc<Reports>,
}

impl<T> PostgresStorage<T> {
    /// A store in the schema `name` of the database `config` names, connecting over `tls`:
    /// [`NoTls`](tokio_postgres::NoTls) for a plain connection, or the caller's own connector. Nothing connects
    /// until the client opens the engine, and the connector is cloned for each connection.
    ///
    /// `name` must be a lowercase identifier of at most 63 characters and not start with `pg_`: the engine creates
    /// the schema, so it refuses a name it can't write into SQL as a plain identifier.
    pub fn new(config: Config, tls: T, name: impl Into<String>) -> PostgresStorage<T> {
        PostgresStorage {
            config,
            tls,
            name: name.into(),
            timeout: TIMEOUT,
            conn: Mutex::new(None),
            reports: Arc::default(),
        }
    }

    /// `timeout` instead of [`TIMEOUT`] for each operation, for tests that must not wait ten seconds.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn timeout(mut self, timeout: Duration) -> PostgresStorage<T> {
        self.timeout = timeout;
        self
    }

    /// The cached connection, unless the server has ended it.
    fn take(&self) -> Option<Client> {
        let mut conn = self.conn.lock().unwrap_or_else(PoisonError::into_inner);
        conn.take().filter(|client| !client.is_closed())
    }

    /// Keeps `client` for the next call, unless the server has ended it meanwhile.
    fn keep(&self, client: Client) {
        if !client.is_closed() {
            *self.conn.lock().unwrap_or_else(PoisonError::into_inner) = Some(client);
        }
    }

    /// The cached connection, or a new one, with this session's schema and tables created and the version checked.
    async fn open_conn(&self) -> io::Result<Client>
    where
        T: MakeTlsConnect<Socket> + Clone + Send + Sync + 'static,
        T::Stream: Send + 'static,
        T::TlsConnect: Send,
        <T::TlsConnect as TlsConnect<Socket>>::Future: Send,
    {
        if !valid_name(&self.name) {
            let why = format!(
                "{:?} can't be a schema: lowercase letters, digits and underscores, at most 63, not starting with \
                 pg_",
                self.name
            );
            return Err(io::Error::new(io::ErrorKind::InvalidInput, why));
        }
        if let Some(client) = self.take() {
            return Ok(client);
        }
        let mut config = self.config.clone();
        // The caller's own limit wins; without one, a server that never answers doesn't hang the store thread.
        if config.get_connect_timeout().is_none() {
            config.connect_timeout(TIMEOUT);
        }
        let (client, connection) = config.connect(self.tls.clone()).await.map_err(io_error)?;
        // The task drives the socket for as long as the connection lives; it ends when the server or the network
        // closes it, and the next call opens another. Reported, as every write after it is retried on the new one.
        let reports = self.reports.clone();
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                reports.report(io_error(e));
            }
        });
        match tokio::time::timeout(self.timeout, self.prepare(&client)).await {
            Ok(result) => result?,
            Err(_) => return Err(timed_out(self.timeout)),
        }
        Ok(client)
    }

    /// Creates this session's schema and tables and checks the version, retrying once when another process created
    /// them at the same moment.
    async fn prepare(&self, client: &Client) -> io::Result<()> {
        let mut attempt = self.prepare_schema(client).await;
        // Two processes opening at once can race on creating the schema or a table: the loser retries once.
        if matches!(&attempt, Err(e) if raced(e)) {
            attempt = self.prepare_schema(client).await;
        }
        attempt.map_err(io_error)?;
        let newer = client
            .query_opt("SELECT number FROM version WHERE number > $1 LIMIT 1", &[&VERSION])
            .await
            .map_err(io_error)?;
        if newer.is_some() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, NEWER));
        }
        client
            .execute("INSERT INTO version (number) VALUES ($1) ON CONFLICT (number) DO NOTHING", &[&VERSION])
            .await
            .map_err(io_error)?;
        Ok(())
    }

    /// The schema and tables, the part a concurrent creator can make fail with a duplicate-object error.
    async fn prepare_schema(&self, client: &Client) -> Result<(), tokio_postgres::Error> {
        let schema = quoted(&self.name);
        client.batch_execute(&format!("CREATE SCHEMA IF NOT EXISTS {schema}")).await?;
        client.batch_execute(&format!("SET search_path TO {schema}")).await?;
        client.batch_execute(SCHEMA).await
    }

    /// Keeps `client` for the next call and returns `result`, unless the operation ran out of time (`None`), which
    /// drops the connection instead: the server may still be working on it, and its answer would be read as the
    /// answer to the next request.
    fn keep_after<R>(&self, client: Client, result: Option<io::Result<R>>) -> io::Result<R> {
        match result {
            Some(result) => {
                self.keep(client);
                result
            }
            None => Err(timed_out(self.timeout)),
        }
    }
}

/// Runs `operation` within `limit`; `None` when it ran out of time.
async fn limited<R>(limit: Duration, operation: impl Future<Output = io::Result<R>> + Send) -> Option<io::Result<R>> {
    tokio::time::timeout(limit, operation).await.ok()
}

/// Whether `name` is a schema this engine may create: a lowercase SQL identifier of at most 63 characters, not one
/// of PostgreSQL's own (`pg_`).
fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    let starts = matches!(chars.next(), Some(c) if c.is_ascii_lowercase() || c == '_');
    starts
        && name.len() <= 63
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !name.starts_with("pg_")
}

/// `name` as a quoted SQL identifier: `valid_name` has accepted it, and quoting keeps it from reading as anything
/// else.
fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn timed_out(limit: Duration) -> io::Error {
    io::Error::new(io::ErrorKind::TimedOut, format!("postgres: no answer within {limit:?}"))
}

/// Whether `e` is what a second process creating the same schema or table at the same moment gets.
fn raced(e: &tokio_postgres::Error) -> bool {
    matches!(e.code(), Some(code) if *code == SqlState::UNIQUE_VIOLATION || *code == SqlState::DUPLICATE_TABLE)
}

fn io_error(e: tokio_postgres::Error) -> io::Error {
    let kind = if e.is_closed() { io::ErrorKind::ConnectionAborted } else { io::ErrorKind::Other };
    io::Error::new(kind, format!("postgres: {e}"))
}

/// A column of `row`, with a type the table doesn't have reported instead of panicking.
fn column<'a, T: FromSql<'a>>(row: &'a Row, name: &str) -> io::Result<T> {
    row.try_get(name).map_err(io_error)
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
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

/// A text value PostgreSQL can hold: it has no NUL byte, and neither can a stored name or number.
fn storable(text: &str) -> bool {
    !text.contains('\0')
}

async fn load_session(client: &Client) -> io::Result<Option<SessionData>> {
    let row = client
        .query_opt(
            "SELECT dc_id, server_address, port, api_id, test_mode, auth_key, user_id, is_bot FROM sessions LIMIT 1",
            &[],
        )
        .await
        .map_err(io_error)?;
    let Some(row) = row else { return Ok(None) };
    let key: Option<Vec<u8>> = column(&row, "auth_key")?;
    let Some(auth_key) = key.and_then(|k| <[u8; 256]>::try_from(k).ok()) else { return Ok(None) };
    let mut s = SessionData::new(column(&row, "dc_id")?, auth_key);
    let address: Option<String> = column(&row, "server_address")?;
    let port: Option<i32> = column(&row, "port")?;
    let ip = address.and_then(|a| a.parse::<IpAddr>().ok());
    let port = port.and_then(|p| u16::try_from(p).ok()).filter(|&p| p != 0);
    s.addr = ip.zip(port).map(|(ip, port)| SocketAddr::new(ip, port));
    s.api_id = column::<Option<i32>>(&row, "api_id")?.unwrap_or(0);
    s.test_mode = column::<Option<bool>>(&row, "test_mode")?.unwrap_or(false);
    s.user_id = column::<Option<i64>>(&row, "user_id")?.unwrap_or(0);
    s.is_bot = column::<Option<bool>>(&row, "is_bot")?.unwrap_or(false);
    Ok(Some(s))
}

async fn save_session(client: &mut Client, s: &SessionData) -> io::Result<()> {
    let (address, port) = match s.addr {
        Some(addr) => (Some(addr.ip().to_string()), Some(i32::from(addr.port()))),
        None => (None, None),
    };
    let date = now();
    // `ToSql` is implemented for `&[u8]`, not `[u8]`, so the slice goes in behind its own reference.
    let key = &s.auth_key[..];
    let tx = client.transaction().await.map_err(io_error)?;
    tx.execute("DELETE FROM sessions", &[]).await.map_err(io_error)?;
    tx.execute(
        "INSERT INTO sessions (dc_id, server_address, port, api_id, test_mode, auth_key, date, user_id, is_bot) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        &[&s.dc_id, &address, &port, &s.api_id, &s.test_mode, &key, &date, &s.user_id, &s.is_bot],
    )
    .await
    .map_err(io_error)?;
    tx.commit().await.map_err(io_error)
}

async fn load_state(client: &Client) -> io::Result<Option<StoredState>> {
    let saved =
        client.query_opt("SELECT owner, salt, time_offset FROM vaegram WHERE id = 0", &[]).await.map_err(io_error)?;
    // This schema is ours alone and `save_state` writes both tables in one transaction, so a state without its
    // owner can't be there.
    let Some(row) = saved else { return Ok(None) };
    let salt = Salt { value: column(&row, "salt")?, time_offset: column(&row, "time_offset")? };
    let owner = column::<i64>(&row, "owner")?;
    let rows = client.query("SELECT id, pts, qts, date, seq FROM update_state", &[]).await.map_err(io_error)?;
    let mut update: Option<UpdateState> = None;
    let mut channels = HashMap::new();
    for row in &rows {
        let id: i64 = column(row, "id")?;
        let pts = column::<Option<i32>>(row, "pts")?.unwrap_or_default();
        if id == 0 {
            update = Some(UpdateState {
                pts,
                qts: column::<Option<i32>>(row, "qts")?.unwrap_or_default(),
                date: column::<Option<i32>>(row, "date")?.unwrap_or_default(),
                seq: column::<Option<i32>>(row, "seq")?.unwrap_or_default(),
                channels: HashMap::new(),
            });
        } else if id <= -CHANNEL_OFFSET {
            channels.insert(get_channel_id(id), pts);
        }
    }
    if let Some(update) = &mut update {
        update.channels = channels;
    }
    let update = update.filter(|u| u.date != 0);
    Ok(Some(StoredState { owner, salt, update }))
}

async fn save_state(client: &mut Client, state: &StoredState) -> io::Result<()> {
    let tx = client.transaction().await.map_err(io_error)?;
    tx.execute(
        "INSERT INTO vaegram (id, owner, salt, time_offset) VALUES (0, $1, $2, $3) \
         ON CONFLICT (id) DO UPDATE SET owner = EXCLUDED.owner, salt = EXCLUDED.salt, \
         time_offset = EXCLUDED.time_offset",
        &[&state.owner, &state.salt.value, &state.salt.time_offset],
    )
    .await
    .map_err(io_error)?;
    tx.execute("DELETE FROM update_state", &[]).await.map_err(io_error)?;
    if let Some(u) = state.update.as_ref().filter(|u| u.date != 0) {
        tx.execute(
            "INSERT INTO update_state (id, pts, qts, date, seq) VALUES (0, $1, $2, $3, $4)",
            &[&u.pts, &u.qts, &u.date, &u.seq],
        )
        .await
        .map_err(io_error)?;
        let channel = tx.prepare("INSERT INTO update_state (id, pts) VALUES ($1, $2)").await.map_err(io_error)?;
        for (id, pts) in &u.channels {
            tx.execute(&channel, &[&get_channel_id(*id), pts]).await.map_err(io_error)?;
        }
    }
    tx.commit().await.map_err(io_error)
}

async fn update_peers(client: &mut Client, peers: &[PeerRecord]) -> io::Result<()> {
    if peers.is_empty() {
        return Ok(());
    }
    let tx = client.transaction().await.map_err(io_error)?;
    let upsert = tx.prepare(UPSERT_PEER).await.map_err(io_error)?;
    let forget = tx.prepare("DELETE FROM usernames WHERE id = $1").await.map_err(io_error)?;
    let add =
        tx.prepare("INSERT INTO usernames (id, username, position) VALUES ($1, $2, $3)").await.map_err(io_error)?;
    for p in peers {
        let kind = peer_type(p.id);
        // A NUL byte can't be in a PostgreSQL text value: such a number is dropped and such a name skipped, as no
        // lookup could ever find them; neither fails the batch.
        let phone = p.phone_number.as_ref().filter(|number| storable(number));
        tx.execute(&upsert, &[&p.id, &p.access_hash, &kind, &phone, &p.updated]).await.map_err(io_error)?;
        tx.execute(&forget, &[&p.id]).await.map_err(io_error)?;
        for (position, username) in p.usernames.iter().filter(|username| storable(username)).enumerate() {
            let position = i32::try_from(position).unwrap_or(i32::MAX);
            tx.execute(&add, &[&p.id, username, &position]).await.map_err(io_error)?;
        }
    }
    tx.commit().await.map_err(io_error)
}

async fn get_peer(client: &Client, id: i64) -> io::Result<Option<PeerRecord>> {
    let Some(row) = client
        .query_opt("SELECT id, access_hash, phone_number, last_update_on FROM peers WHERE id = $1", &[&id])
        .await
        .map_err(io_error)?
    else {
        return Ok(None);
    };
    let mut p = PeerRecord::new(column(&row, "id")?, column::<Option<i64>>(&row, "access_hash")?.unwrap_or(0));
    (p.phone_number, p.updated) = (column(&row, "phone_number")?, column(&row, "last_update_on")?);
    let names = client
        .query("SELECT username FROM usernames WHERE id = $1 ORDER BY position", &[&id])
        .await
        .map_err(io_error)?;
    p.usernames = names.iter().map(|row| column::<String>(row, "username")).collect::<io::Result<_>>()?;
    Ok(Some(p))
}

// The most recently updated claim wins, the larger id on a tie, as in every engine.
async fn get_peer_where(client: &Client, query: &str, key: &str) -> io::Result<Option<PeerRecord>> {
    match client.query_opt(query, &[&key]).await.map_err(io_error)? {
        Some(row) => get_peer(client, column(&row, "id")?).await,
        None => Ok(None),
    }
}

async fn export_peers(client: &Client, limit: Option<usize>) -> io::Result<Vec<PeerRecord>> {
    let limit = limit.map(|n| i64::try_from(n).unwrap_or(i64::MAX));
    let rows = client
        .query("SELECT id FROM peers ORDER BY last_update_on DESC, id ASC LIMIT $1", &[&limit])
        .await
        .map_err(io_error)?;
    let ids: Vec<i64> = rows.iter().map(|row| column(row, "id")).collect::<io::Result<_>>()?;
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    // One query for the peers and one for their usernames, rather than a round trip for each peer.
    let peers = client
        .query("SELECT id, access_hash, phone_number, last_update_on FROM peers WHERE id = ANY($1)", &[&ids])
        .await
        .map_err(io_error)?;
    let names = client
        .query("SELECT id, username FROM usernames WHERE id = ANY($1) ORDER BY id, position", &[&ids])
        .await
        .map_err(io_error)?;
    let mut records: HashMap<i64, PeerRecord> = HashMap::with_capacity(peers.len());
    for row in &peers {
        let mut p = PeerRecord::new(column(row, "id")?, column::<Option<i64>>(row, "access_hash")?.unwrap_or(0));
        (p.phone_number, p.updated) = (column(row, "phone_number")?, column(row, "last_update_on")?);
        records.insert(p.id, p);
    }
    for row in &names {
        let id: i64 = column(row, "id")?;
        if let Some(p) = records.get_mut(&id) {
            p.usernames.push(column(row, "username")?);
        }
    }
    Ok(ids.into_iter().filter_map(|id| records.remove(&id)).collect())
}

async fn clear_peers(client: &Client) -> io::Result<()> {
    client.batch_execute("DELETE FROM usernames; DELETE FROM peers;").await.map_err(io_error)
}

async fn delete(client: &Client) -> io::Result<()> {
    client.batch_execute(DELETE_ALL).await.map_err(io_error)
}

impl<T> fmt::Debug for PostgresStorage<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `Config`'s own `Debug` hides the password.
        f.debug_struct("PostgresStorage")
            .field("config", &self.config)
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl<T> Storage for PostgresStorage<T>
where
    T: MakeTlsConnect<Socket> + Clone + Send + Sync + 'static,
    T::Stream: Send + 'static,
    T::TlsConnect: Send,
    <T::TlsConnect as TlsConnect<Socket>>::Future: Send,
{
    fn set_reporter(&self, reporter: StorageReporter) {
        self.reports.set(reporter);
    }

    fn open(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let client = self.open_conn().await?;
            self.keep(client);
            Ok(())
        })
    }

    fn close(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            self.conn.lock().unwrap_or_else(PoisonError::into_inner).take();
            Ok(())
        })
    }

    fn delete(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let client = self.open_conn().await?;
            let result = limited(self.timeout, delete(&client)).await;
            self.keep_after(client, result)
        })
    }

    fn load_session(&self) -> StorageFuture<'_, Option<SessionData>> {
        Box::pin(async move {
            let client = self.open_conn().await?;
            let result = limited(self.timeout, load_session(&client)).await;
            self.keep_after(client, result)
        })
    }

    fn save_session<'a>(&'a self, session: &'a SessionData) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let mut client = self.open_conn().await?;
            let result = limited(self.timeout, save_session(&mut client, session)).await;
            self.keep_after(client, result)
        })
    }

    fn load_state(&self) -> StorageFuture<'_, Option<StoredState>> {
        Box::pin(async move {
            let client = self.open_conn().await?;
            let result = limited(self.timeout, load_state(&client)).await;
            self.keep_after(client, result)
        })
    }

    fn save_state<'a>(&'a self, state: &'a StoredState) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let mut client = self.open_conn().await?;
            let result = limited(self.timeout, save_state(&mut client, state)).await;
            self.keep_after(client, result)
        })
    }

    fn update_peers<'a>(&'a self, peers: &'a [PeerRecord]) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let mut client = self.open_conn().await?;
            let result = limited(self.timeout, update_peers(&mut client, peers)).await;
            self.keep_after(client, result)
        })
    }

    fn get_peer_by_id(&self, id: i64) -> StorageFuture<'_, Option<PeerRecord>> {
        Box::pin(async move {
            let client = self.open_conn().await?;
            let result = limited(self.timeout, get_peer(&client, id)).await;
            self.keep_after(client, result)
        })
    }

    fn get_peer_by_username<'a>(&'a self, username: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        Box::pin(async move {
            // A NUL byte can't be in a PostgreSQL text value, and no stored name has one.
            if !storable(username) {
                return Ok(None);
            }
            let query = "SELECT p.id FROM peers p JOIN usernames u ON p.id = u.id WHERE u.username = $1 \
                         ORDER BY p.last_update_on DESC, p.id DESC LIMIT 1";
            let client = self.open_conn().await?;
            let result = limited(self.timeout, get_peer_where(&client, query, username)).await;
            self.keep_after(client, result)
        })
    }

    fn get_peer_by_phone_number<'a>(&'a self, phone_number: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        Box::pin(async move {
            if !storable(phone_number) {
                return Ok(None);
            }
            let query = "SELECT id FROM peers WHERE phone_number = $1 ORDER BY last_update_on DESC, id DESC LIMIT 1";
            let client = self.open_conn().await?;
            let result = limited(self.timeout, get_peer_where(&client, query, phone_number)).await;
            self.keep_after(client, result)
        })
    }

    fn export_peers(&self, limit: Option<usize>) -> StorageFuture<'_, Vec<PeerRecord>> {
        Box::pin(async move {
            let client = self.open_conn().await?;
            let result = limited(self.timeout, export_peers(&client, limit)).await;
            self.keep_after(client, result)
        })
    }

    fn clear_peers(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let client = self.open_conn().await?;
            let result = limited(self.timeout, clear_peers(&client)).await;
            self.keep_after(client, result)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::time::Duration;

    use tokio_postgres::{Config, NoTls};

    use super::{PostgresStorage, valid_name};
    use crate::storage::{SessionData, Storage, conformance};

    /// The URL of a server the test may wipe, or `None` when it isn't set outside CI.
    fn url() -> Option<String> {
        match std::env::var("VAEGRAM_POSTGRES_URL") {
            Ok(url) => Some(url),
            Err(_) => {
                // The CI job that runs these has a server; without the variable they would pass having tested
                // nothing.
                assert!(std::env::var_os("CI").is_none(), "set VAEGRAM_POSTGRES_URL");
                None
            }
        }
    }

    /// `url` as a config, refused unless it names a database ending in `_test`: these tests wipe it, so a URL
    /// pointing at a real database must fail before anything connects.
    fn test_config(url: &str) -> Result<Config, String> {
        let config = Config::from_str(url).map_err(|e| format!("VAEGRAM_POSTGRES_URL isn't a PostgreSQL URL: {e}"))?;
        let named = config.get_dbname().is_some_and(|name| name.ends_with("_test"));
        if !named {
            return Err("VAEGRAM_POSTGRES_URL must name a database ending in _test: the test wipes it".into());
        }
        Ok(config)
    }

    fn storage(config: Config, name: &str) -> PostgresStorage<NoTls> {
        PostgresStorage::new(config, NoTls, name)
    }

    #[test]
    fn a_plain_connector_is_a_storage() {
        fn assert_storage<T: Storage>(_: &T) {}
        assert_storage(&PostgresStorage::new(Config::new(), NoTls, "bot"));
    }

    #[test]
    fn a_url_that_names_a_real_database_is_refused() {
        let error = test_config("postgresql://vaegram@localhost/vaegram").unwrap_err();
        assert!(error.contains("_test"), "{error}");
        assert!(test_config("postgresql://vaegram@localhost/vaegram_test").is_ok());
        assert!(test_config("not a url").is_err());
    }

    #[test]
    fn a_schema_name_must_be_an_identifier() {
        for name in ["bot_1", "_", "a", "vaegram_conformance_test"] {
            assert!(valid_name(name), "{name}");
        }
        for name in ["", "Bot", "a-b", "a\"b", "pg_x", "9x", "a b"] {
            assert!(!valid_name(name), "{name}");
        }
        assert!(valid_name(&"a".repeat(63)));
        assert!(!valid_name(&"a".repeat(64)));
    }

    #[tokio::test]
    async fn debug_hides_the_password() {
        let mut config = Config::new();
        config.host("127.0.0.1").port(1).user("vaegram").password("hunter2").dbname("vaegram");
        let storage = PostgresStorage::new(config, NoTls, "vaegram");
        let debug = format!("{storage:?}");
        let error = storage.open().await.unwrap_err().to_string();
        assert!(!debug.contains("hunter2") && debug.contains("vaegram"), "{debug}");
        assert!(!error.contains("hunter2"), "{error}");
    }

    #[tokio::test]
    async fn a_bad_schema_name_fails_open() {
        let storage = PostgresStorage::new(Config::new(), NoTls, "pg_bot");
        let error = storage.open().await.unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput, "{error}");
    }

    #[tokio::test]
    #[ignore = "needs VAEGRAM_POSTGRES_URL naming a database ending in _test, which it wipes"]
    async fn conformance_on_a_live_server() {
        let Some(url) = url() else { return };
        let storage = storage(test_config(&url).unwrap(), "vaegram_conformance_test");
        // `check` starts on an engine that holds nothing: whatever a previous run left goes first.
        storage.delete().await.unwrap();
        conformance::check(&storage).await;
    }

    #[tokio::test]
    #[ignore = "needs VAEGRAM_POSTGRES_URL naming a database ending in _test, which it wipes"]
    async fn two_names_on_one_database_keep_their_own_sessions() {
        let Some(url) = url() else { return };
        let a = storage(test_config(&url).unwrap(), "vaegram_two_a_test");
        let b = storage(test_config(&url).unwrap(), "vaegram_two_b_test");
        a.delete().await.unwrap();
        b.delete().await.unwrap();
        a.save_session(&SessionData::new(2, [1; 256])).await.unwrap();
        b.save_session(&SessionData::new(4, [2; 256])).await.unwrap();
        assert_eq!(a.load_session().await.unwrap().map(|s| s.dc_id), Some(2));
        assert_eq!(b.load_session().await.unwrap().map(|s| s.dc_id), Some(4));
    }

    #[tokio::test]
    #[ignore = "needs VAEGRAM_POSTGRES_URL naming a database ending in _test, which it wipes"]
    async fn reconnects_after_the_server_drops() {
        let Some(url) = url() else { return };
        let mut config = test_config(&url).unwrap();
        config.application_name("vaegram-reconnect-test");
        let storage = PostgresStorage::new(config, NoTls, "vaegram_reconnect_test");
        let reports = conformance::collect_reports(&storage);
        storage.save_session(&SessionData::new(2, [1; 256])).await.unwrap();

        // A second connection ends the storage's backend, as a server restart would.
        let (killer, connection) = test_config(&url).unwrap().connect(NoTls).await.unwrap();
        tokio::spawn(connection);
        let ended = killer
            .execute(
                "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
                 WHERE datname = current_database() AND application_name = 'vaegram-reconnect-test' \
                 AND pid <> pg_backend_pid()",
                &[],
            )
            .await
            .unwrap();
        assert_eq!(ended, 1, "exactly the storage's backend is ended");

        // The drop is reported and the backend leaves `pg_stat_activity` before the save that must work.
        let mut gone = false;
        let mut reported = false;
        for _ in 0..100 {
            let left: i64 = killer
                .query_one(
                    "SELECT count(*) FROM pg_stat_activity \
                     WHERE datname = current_database() AND application_name = 'vaegram-reconnect-test'",
                    &[],
                )
                .await
                .unwrap()
                .get(0);
            gone = left == 0;
            reported = !reports.lock().unwrap().is_empty();
            if gone && reported {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(gone, "the old backend is gone");
        assert!(reported, "the drop is reported");
        storage.save_session(&SessionData::new(4, [2; 256])).await.unwrap();
        assert_eq!(storage.load_session().await.unwrap().map(|s| s.dc_id), Some(4));
    }

    #[tokio::test]
    #[ignore = "needs VAEGRAM_POSTGRES_URL naming a database ending in _test, which it wipes"]
    async fn a_blocked_statement_times_out() {
        let Some(url) = url() else { return };
        let mut config = test_config(&url).unwrap();
        let locker_config = config.clone();
        config.application_name("vaegram-timeout-test");
        let storage = PostgresStorage::new(config, NoTls, "vaegram_timeout_test").timeout(Duration::from_millis(500));
        storage.open().await.unwrap();

        // A second connection holds the table, so the save waits for it.
        let (locker, connection) = locker_config.connect(NoTls).await.unwrap();
        tokio::spawn(connection);
        locker
            .batch_execute(
                "BEGIN; SET search_path TO \"vaegram_timeout_test\"; LOCK TABLE sessions IN ACCESS EXCLUSIVE MODE",
            )
            .await
            .unwrap();
        let error = storage.save_session(&SessionData::new(2, [1; 256])).await.unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut, "{error}");

        // The server is still running the statement the storage gave up on: end it before releasing the table.
        locker
            .execute(
                "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
                 WHERE datname = current_database() AND application_name = 'vaegram-timeout-test' \
                 AND pid <> pg_backend_pid()",
                &[],
            )
            .await
            .unwrap();
        locker.batch_execute("ROLLBACK").await.unwrap();
        // The connection that timed out was dropped, so this one opens a new connection and goes through.
        storage.save_session(&SessionData::new(2, [1; 256])).await.unwrap();
    }
}
