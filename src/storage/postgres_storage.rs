use std::collections::HashMap;
use std::fmt;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use tokio_postgres::tls::{MakeTlsConnect, TlsConnect};
use tokio_postgres::types::FromSql;
use tokio_postgres::{Client, Config, Row, Socket};

use super::engine::{PeerRecord, Reports, Storage, StorageFuture, StorageReporter, StoredState};
use super::{SessionData, UpdateState};
use crate::session::Salt;
use crate::utils::{CHANNEL_OFFSET, get_channel_id};

const VERSION: i64 = 1;

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

/// Keeps everything in a PostgreSQL database, in tables that mirror the session layout of the SQLite engine
/// (`sqlite_storage.rs`): the same fields, so a session moves between the two.
///
/// The constructor only holds the [`Config`] and the TLS connector. Connecting, spawning the connection's own task
/// and reconnecting all happen when the client opens the engine and on every call after, on the store thread's
/// runtime, so the caller's runtime is never blocked on and a dropped client still saves.
///
/// A connection the server ended is left behind and the next call opens a new one. Usernames are matched in
/// lowercase, as written by this library and the Python MTProto libraries.
pub struct PostgresStorage<T> {
    config: Config,
    tls: T,
    conn: Mutex<Option<Client>>,
    reports: Arc<Reports>,
}

impl<T> PostgresStorage<T> {
    /// A store in the database `config` names, connecting over `tls`: [`NoTls`](tokio_postgres::NoTls) for a plain
    /// connection, or the caller's own connector. Nothing connects until the client opens the engine, and the
    /// connector is cloned for each connection.
    pub fn new(config: Config, tls: T) -> PostgresStorage<T> {
        PostgresStorage { config, tls, conn: Mutex::new(None), reports: Arc::default() }
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

    /// The cached connection, or a new one, with the tables created and the schema version checked.
    async fn open_conn(&self) -> io::Result<Client>
    where
        T: MakeTlsConnect<Socket> + Clone + Send + Sync + 'static,
        T::Stream: Send + 'static,
        T::TlsConnect: Send,
        <T::TlsConnect as TlsConnect<Socket>>::Future: Send,
    {
        if let Some(client) = self.take() {
            return Ok(client);
        }
        let (client, connection) = self.config.connect(self.tls.clone()).await.map_err(io_error)?;
        // The task drives the socket for as long as the connection lives; it ends when the server or the network
        // closes it, and the next call opens another. Reported, as every write after it is retried on the new one.
        let reports = self.reports.clone();
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                reports.report(io_error(e));
            }
        });
        client.batch_execute(SCHEMA).await.map_err(io_error)?;
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
        Ok(client)
    }
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
    let saved = client
        .query_opt("SELECT owner, salt, time_offset FROM vaegram WHERE id = 0", &[])
        .await
        .map_err(io_error)?;
    let saved = match saved {
        Some(row) => {
            let salt = Salt { value: column(&row, "salt")?, time_offset: column(&row, "time_offset")? };
            Some((column::<i64>(&row, "owner")?, salt))
        }
        None => None,
    };
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
    Ok(match saved {
        Some((owner, salt)) => Some(StoredState { owner, salt, update }),
        // A database another library wrote: its account owns the update state.
        None if update.is_some() => {
            let owner = load_session(client).await?.map_or(0, |s| s.user_id);
            Some(StoredState { owner, salt: Salt::default(), update })
        }
        None => None,
    })
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
    let add = tx.prepare("INSERT INTO usernames (id, username, position) VALUES ($1, $2, $3)").await.map_err(io_error)?;
    for p in peers {
        let kind = peer_type(p.id);
        tx.execute(&upsert, &[&p.id, &p.access_hash, &kind, &p.phone_number, &p.updated]).await.map_err(io_error)?;
        tx.execute(&forget, &[&p.id]).await.map_err(io_error)?;
        for (position, username) in p.usernames.iter().enumerate() {
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
        f.debug_struct("PostgresStorage").field("config", &self.config).finish_non_exhaustive()
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
            let result = delete(&client).await;
            self.keep(client);
            result
        })
    }

    fn load_session(&self) -> StorageFuture<'_, Option<SessionData>> {
        Box::pin(async move {
            let client = self.open_conn().await?;
            let result = load_session(&client).await;
            self.keep(client);
            result
        })
    }

    fn save_session<'a>(&'a self, session: &'a SessionData) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let mut client = self.open_conn().await?;
            let result = save_session(&mut client, session).await;
            self.keep(client);
            result
        })
    }

    fn load_state(&self) -> StorageFuture<'_, Option<StoredState>> {
        Box::pin(async move {
            let client = self.open_conn().await?;
            let result = load_state(&client).await;
            self.keep(client);
            result
        })
    }

    fn save_state<'a>(&'a self, state: &'a StoredState) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let mut client = self.open_conn().await?;
            let result = save_state(&mut client, state).await;
            self.keep(client);
            result
        })
    }

    fn update_peers<'a>(&'a self, peers: &'a [PeerRecord]) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let mut client = self.open_conn().await?;
            let result = update_peers(&mut client, peers).await;
            self.keep(client);
            result
        })
    }

    fn get_peer_by_id(&self, id: i64) -> StorageFuture<'_, Option<PeerRecord>> {
        Box::pin(async move {
            let client = self.open_conn().await?;
            let result = get_peer(&client, id).await;
            self.keep(client);
            result
        })
    }

    fn get_peer_by_username<'a>(&'a self, username: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        Box::pin(async move {
            let client = self.open_conn().await?;
            let query = "SELECT p.id FROM peers p JOIN usernames u ON p.id = u.id WHERE u.username = $1 \
                         ORDER BY p.last_update_on DESC, p.id DESC LIMIT 1";
            let result = get_peer_where(&client, query, username).await;
            self.keep(client);
            result
        })
    }

    fn get_peer_by_phone_number<'a>(&'a self, phone_number: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        Box::pin(async move {
            let client = self.open_conn().await?;
            let query = "SELECT id FROM peers WHERE phone_number = $1 ORDER BY last_update_on DESC, id DESC LIMIT 1";
            let result = get_peer_where(&client, query, phone_number).await;
            self.keep(client);
            result
        })
    }

    fn export_peers(&self, limit: Option<usize>) -> StorageFuture<'_, Vec<PeerRecord>> {
        Box::pin(async move {
            let client = self.open_conn().await?;
            let result = export_peers(&client, limit).await;
            self.keep(client);
            result
        })
    }

    fn clear_peers(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let client = self.open_conn().await?;
            let result = clear_peers(&client).await;
            self.keep(client);
            result
        })
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::time::Duration;

    use tokio_postgres::{Config, NoTls};

    use super::PostgresStorage;
    use crate::storage::{SessionData, Storage, conformance};

    /// The URL of a server the test may wipe, or `None` when it isn't set outside CI.
    fn url() -> Option<String> {
        match std::env::var("VAEGRAM_POSTGRES_URL") {
            Ok(url) => Some(url),
            None => {
                // The CI job that runs these has a server; without the variable they would pass having tested
                // nothing.
                assert!(std::env::var_os("CI").is_none(), "set VAEGRAM_POSTGRES_URL");
                None
            }
        }
    }

    fn storage(url: &str) -> PostgresStorage<NoTls> {
        let config = Config::from_str(url).expect("VAEGRAM_POSTGRES_URL isn't a PostgreSQL URL");
        PostgresStorage::new(config, NoTls)
    }

    #[test]
    fn a_plain_connector_is_a_storage() {
        fn assert_storage<T: Storage>(_: &T) {}
        assert_storage(&PostgresStorage::new(Config::new(), NoTls));
    }

    #[tokio::test]
    async fn debug_hides_the_password() {
        let mut config = Config::new();
        config.host("127.0.0.1").port(1).user("vaegram").password("hunter2").dbname("vaegram");
        let storage = PostgresStorage::new(config, NoTls);
        let debug = format!("{storage:?}");
        let error = storage.open().await.unwrap_err().to_string();
        assert!(!debug.contains("hunter2") && debug.contains("vaegram"), "{debug}");
        assert!(!error.contains("hunter2"), "{error}");
    }

    #[tokio::test]
    #[ignore = "needs VAEGRAM_POSTGRES_URL, a PostgreSQL server it may wipe"]
    async fn conformance_on_a_live_server() {
        let Some(url) = url() else { return };
        let storage = storage(&url);
        // `check` starts on an engine that holds nothing: whatever a previous run left goes first.
        storage.delete().await.unwrap();
        conformance::check(&storage).await;
    }

    #[tokio::test]
    #[ignore = "needs VAEGRAM_POSTGRES_URL, a PostgreSQL server it may wipe"]
    async fn reconnects_after_the_server_drops() {
        let Some(url) = url() else { return };
        let mut config = Config::from_str(&url).unwrap();
        config.application_name("vaegram-reconnect-test");
        let storage = PostgresStorage::new(config, NoTls);
        storage.save_session(&SessionData::new(2, [1; 256])).await.unwrap();

        // A second connection ends the storage's backend, as a server restart would.
        let (killer, connection) = Config::from_str(&url).unwrap().connect(NoTls).await.unwrap();
        tokio::spawn(connection);
        killer
            .execute(
                "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
                 WHERE application_name = 'vaegram-reconnect-test' AND pid <> pg_backend_pid()",
                &[],
            )
            .await
            .unwrap();

        // The first save after the drop can still find the old connection, so it is the ones after it that must
        // work: the engine opens a new connection once the server has ended the old one.
        let mut saved = false;
        for _ in 0..100 {
            if storage.save_session(&SessionData::new(4, [2; 256])).await.is_ok() {
                saved = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(saved, "the storage never reconnected");
        assert_eq!(storage.load_session().await.unwrap().map(|s| s.dc_id), Some(4));
    }
}
