use std::collections::HashMap;
use std::fmt;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
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
/// How long the cancel request that stops a statement this engine gave up on may take.
const CANCEL_TIMEOUT: Duration = Duration::from_secs(2);

// The tables of `sqlite_storage.rs`, column for column, in PostgreSQL types: a session moves between the two
// engines field for field, which is what keeps the session format the same. `{schema}` is filled in once, by
// `Sql::new`.
const TABLES: &str = "
CREATE TABLE IF NOT EXISTS {schema}.sessions
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

CREATE TABLE IF NOT EXISTS {schema}.peers
(
    id             BIGINT PRIMARY KEY,
    access_hash    BIGINT,
    type           TEXT NOT NULL,
    phone_number   TEXT,
    last_update_on BIGINT NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS {schema}.usernames
(
    id       BIGINT NOT NULL REFERENCES {schema}.peers (id) ON DELETE CASCADE,
    username TEXT NOT NULL,
    position INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS {schema}.update_state
(
    id   BIGINT PRIMARY KEY,
    pts  INTEGER,
    qts  INTEGER,
    date INTEGER,
    seq  INTEGER
);

CREATE TABLE IF NOT EXISTS {schema}.version
(
    number BIGINT PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS {schema}.vaegram
(
    id          INTEGER PRIMARY KEY CHECK (id = 0),
    owner       BIGINT NOT NULL,
    salt        BIGINT NOT NULL,
    time_offset BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_peers_phone_number ON {schema}.peers (phone_number);
CREATE INDEX IF NOT EXISTS idx_peers_last_update_on ON {schema}.peers (last_update_on DESC);
CREATE INDEX IF NOT EXISTS idx_usernames_id ON {schema}.usernames (id);
CREATE INDEX IF NOT EXISTS idx_usernames_username ON {schema}.usernames (username);
";

const NEWER: &str = "this PostgreSQL session needs a newer vaegram";

/// Every statement this engine sends, with this session's schema already written into it.
///
/// Nothing here depends on a session setting: a transaction-mode pooler (PgBouncer, the pooled endpoints of
/// Supabase and Neon) may run the next statement on another connection, so a `SET search_path` would put one
/// session's rows in another's schema. Naming the schema in every statement is what makes the engine safe there.
struct Sql {
    create_schema: String,
    create_tables: String,
    delete_all: String,
    upsert_peer: String,
    forget_usernames: String,
    add_username: String,
    load_session: String,
    clear_sessions: String,
    save_session: String,
    load_owner: String,
    load_update_state: String,
    save_owner: String,
    clear_update_state: String,
    save_update: String,
    save_channel: String,
    newest_version: String,
    record_version: String,
    get_peer: String,
    get_peer_usernames: String,
    peer_by_username: String,
    peer_by_phone_number: String,
    export_ids: String,
    export_peers: String,
    export_usernames: String,
    clear_peers: String,
}

impl Sql {
    fn new(name: &str) -> Sql {
        let schema = quoted(name);
        let fill = |template: &str| template.replace("{schema}", &schema);
        Sql {
            create_schema: fill("CREATE SCHEMA IF NOT EXISTS {schema}"),
            create_tables: fill(TABLES),
            delete_all: fill(
                "DELETE FROM {schema}.usernames; DELETE FROM {schema}.peers; DELETE FROM {schema}.update_state; \
                 DELETE FROM {schema}.sessions; DELETE FROM {schema}.vaegram; DELETE FROM {schema}.version;",
            ),
            upsert_peer: fill(
                "INSERT INTO {schema}.peers (id, access_hash, type, phone_number, last_update_on) \
                 VALUES ($1, $2, $3, $4, $5) \
                 ON CONFLICT (id) DO UPDATE SET access_hash = EXCLUDED.access_hash, type = EXCLUDED.type, \
                 phone_number = EXCLUDED.phone_number, last_update_on = EXCLUDED.last_update_on",
            ),
            forget_usernames: fill("DELETE FROM {schema}.usernames WHERE id = $1"),
            add_username: fill("INSERT INTO {schema}.usernames (id, username, position) VALUES ($1, $2, $3)"),
            load_session: fill(
                "SELECT dc_id, server_address, port, api_id, test_mode, auth_key, user_id, is_bot FROM \
                 {schema}.sessions LIMIT 1",
            ),
            clear_sessions: fill("DELETE FROM {schema}.sessions"),
            save_session: fill(
                "INSERT INTO {schema}.sessions (dc_id, server_address, port, api_id, test_mode, auth_key, date, \
                 user_id, is_bot) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            ),
            load_owner: fill("SELECT owner, salt, time_offset FROM {schema}.vaegram WHERE id = 0"),
            load_update_state: fill("SELECT id, pts, qts, date, seq FROM {schema}.update_state"),
            save_owner: fill(
                "INSERT INTO {schema}.vaegram (id, owner, salt, time_offset) VALUES (0, $1, $2, $3) \
                 ON CONFLICT (id) DO UPDATE SET owner = EXCLUDED.owner, salt = EXCLUDED.salt, \
                 time_offset = EXCLUDED.time_offset",
            ),
            clear_update_state: fill("DELETE FROM {schema}.update_state"),
            save_update: fill("INSERT INTO {schema}.update_state (id, pts, qts, date, seq) VALUES (0, $1, $2, $3, $4)"),
            save_channel: fill("INSERT INTO {schema}.update_state (id, pts) VALUES ($1, $2)"),
            newest_version: fill("SELECT number FROM {schema}.version WHERE number > $1 LIMIT 1"),
            record_version: fill("INSERT INTO {schema}.version (number) VALUES ($1) ON CONFLICT (number) DO NOTHING"),
            get_peer: fill("SELECT id, access_hash, phone_number, last_update_on FROM {schema}.peers WHERE id = $1"),
            get_peer_usernames: fill("SELECT username FROM {schema}.usernames WHERE id = $1 ORDER BY position"),
            peer_by_username: fill(
                "SELECT p.id FROM {schema}.peers p JOIN {schema}.usernames u ON p.id = u.id WHERE u.username = $1 \
                 ORDER BY p.last_update_on DESC, p.id DESC LIMIT 1",
            ),
            peer_by_phone_number: fill(
                "SELECT id FROM {schema}.peers WHERE phone_number = $1 ORDER BY last_update_on DESC, id DESC LIMIT 1",
            ),
            export_ids: fill("SELECT id FROM {schema}.peers ORDER BY last_update_on DESC, id ASC LIMIT $1"),
            export_peers: fill(
                "SELECT id, access_hash, phone_number, last_update_on FROM {schema}.peers WHERE id = ANY($1)",
            ),
            export_usernames: fill(
                "SELECT id, username FROM {schema}.usernames WHERE id = ANY($1) ORDER BY id, position",
            ),
            clear_peers: fill("DELETE FROM {schema}.usernames; DELETE FROM {schema}.peers;"),
        }
    }

    /// Every statement, for the test that checks each one names the schema.
    #[cfg(test)]
    fn statements(&self) -> [&String; 25] {
        [
            &self.create_schema,
            &self.create_tables,
            &self.delete_all,
            &self.upsert_peer,
            &self.forget_usernames,
            &self.add_username,
            &self.load_session,
            &self.clear_sessions,
            &self.save_session,
            &self.load_owner,
            &self.load_update_state,
            &self.save_owner,
            &self.clear_update_state,
            &self.save_update,
            &self.save_channel,
            &self.newest_version,
            &self.record_version,
            &self.get_peer,
            &self.get_peer_usernames,
            &self.peer_by_username,
            &self.peer_by_phone_number,
            &self.export_ids,
            &self.export_peers,
            &self.export_usernames,
            &self.clear_peers,
        ]
    }
}

/// Keeps everything in a PostgreSQL database, in a schema of its own named `name`, in tables that mirror the
/// session layout of the SQLite engine (`sqlite_storage.rs`): the same fields, so a session moves between the two.
///
/// One schema per session: two clients on one database keep their own logins, and the schema is created when the
/// engine opens if it isn't there, so the role needs `CREATE` on the database, or a schema it owns already.
///
/// Every statement names the schema and none sets session state, so the engine also works behind a transaction-mode
/// pooler (PgBouncer, the pooled endpoints of Supabase and Neon), which may run each statement on a different
/// server connection.
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
    sql: Sql,
    limits: String,
    timeout: Duration,
    prepared: AtomicBool,
    conn: Mutex<Option<Client>>,
    reports: Arc<Reports>,
}

impl<T> PostgresStorage<T> {
    /// A store in the schema `name` of the database `config` names, connecting over `tls`:
    /// [`NoTls`](tokio_postgres::NoTls) for a plain connection, or the caller's own connector. Nothing connects
    /// until the client opens the engine, and the connector is cloned for each connection.
    ///
    /// `name` must be a lowercase identifier of at most 63 characters, not starting with `pg_` and not `public` or
    /// `information_schema`: the engine creates the schema, so it refuses a name it can't write into SQL as a plain
    /// identifier or one every database already has.
    pub fn new(config: Config, tls: T, name: impl Into<String>) -> PostgresStorage<T> {
        let name = name.into();
        PostgresStorage {
            config,
            tls,
            sql: Sql::new(&name),
            limits: transaction_limits(TIMEOUT),
            name,
            timeout: TIMEOUT,
            prepared: AtomicBool::new(false),
            conn: Mutex::new(None),
            reports: Arc::default(),
        }
    }

    /// `timeout` instead of [`TIMEOUT`] for each operation, for tests that must not wait ten seconds.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn timeout(mut self, timeout: Duration) -> PostgresStorage<T> {
        self.timeout = timeout;
        self.limits = transaction_limits(timeout);
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
}

impl<T> PostgresStorage<T>
where
    T: MakeTlsConnect<Socket> + Clone + Send + Sync + 'static,
    T::Stream: Send + 'static,
    T::TlsConnect: Send,
    <T::TlsConnect as TlsConnect<Socket>>::Future: Send,
{
    /// The cached connection, or a new one, with this session's schema and tables created and the version checked.
    async fn open_conn(&self) -> io::Result<Client> {
        if !valid_name(&self.name) {
            let why = format!(
                "{:?} can't be a schema: a lowercase identifier of at most 63 characters, not starting with pg_, \
                 not public or information_schema",
                self.name
            );
            return Err(io::Error::new(io::ErrorKind::InvalidInput, why));
        }
        if let Some(client) = self.take() {
            return Ok(client);
        }
        // The whole connect, not just the TCP handshake: DNS, the TLS handshake and the login can hang as well as
        // the socket can.
        let connecting = self.config.connect(self.tls.clone());
        let (client, connection) = match tokio::time::timeout(self.timeout, connecting).await {
            Ok(result) => result.map_err(io_error)?,
            Err(_) => return Err(timed_out(self.timeout)),
        };
        // The task drives the socket for as long as the connection lives; it ends when the server or the network
        // closes it, and the next call opens another. Reported, as every write after it is retried on the new one.
        let reports = self.reports.clone();
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                reports.report(io_error(e));
            }
        });
        // Once per engine: the schema and tables outlive a connection, so a reconnect has no reason to create them
        // again. An attempt that fails leaves the flag clear and the next call tries again.
        if !self.prepared.load(Ordering::Acquire) {
            match tokio::time::timeout(self.timeout, self.prepare(&client)).await {
                Ok(result) => result?,
                Err(_) => {
                    self.cancel(&client).await;
                    return Err(timed_out(self.timeout));
                }
            }
            self.prepared.store(true, Ordering::Release);
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
        let newer = client.query_opt(self.sql.newest_version.as_str(), &[&VERSION]).await.map_err(io_error)?;
        if newer.is_some() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, NEWER));
        }
        client.execute(self.sql.record_version.as_str(), &[&VERSION]).await.map_err(io_error)?;
        Ok(())
    }

    /// The schema and tables, the part a concurrent creator can make fail with a duplicate-object error.
    async fn prepare_schema(&self, client: &Client) -> Result<(), tokio_postgres::Error> {
        client.batch_execute(self.sql.create_schema.as_str()).await?;
        client.batch_execute(self.sql.create_tables.as_str()).await
    }

    /// Asks the server to stop the statement `client` is running, so a connection this engine gave up on doesn't
    /// stay parked until the statement answers. This is what stops a read, which has no transaction to carry
    /// [`transaction_limits`]; a write is also cut short by those. Bounded by [`CANCEL_TIMEOUT`] and best effort:
    /// the client is dropped either way.
    async fn cancel(&self, client: &Client) {
        let token = client.cancel_token();
        let _ = tokio::time::timeout(CANCEL_TIMEOUT, token.cancel_query(self.tls.clone())).await;
    }

    /// Keeps `client` for the next call and returns `result`, unless the operation ran out of time (`None`).
    ///
    /// A connection that ran out of time is dropped rather than kept: the next request on it would queue behind
    /// the statement that is still running. Before dropping it the statement is cancelled, so the server stops
    /// working on it and its transaction is rolled back instead of committing after a newer write; the one thing a
    /// cancel can't recall is a commit already sent, which can only carry what was written before the timeout.
    async fn keep_after<R>(&self, client: Client, result: Option<io::Result<R>>) -> io::Result<R> {
        match result {
            Some(result) => {
                self.keep(client);
                result
            }
            None => {
                self.cancel(&client).await;
                Err(timed_out(self.timeout))
            }
        }
    }
}

/// Runs `operation` within `limit`; `None` when it ran out of time.
async fn limited<R>(limit: Duration, operation: impl Future<Output = io::Result<R>> + Send) -> Option<io::Result<R>> {
    tokio::time::timeout(limit, operation).await.ok()
}

/// Whether `name` is a schema this engine may create: a lowercase SQL identifier of at most 63 characters, not one
/// of PostgreSQL's own (`pg_`) and not one every database already has (`public`, `information_schema`).
fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    let starts = matches!(chars.next(), Some(c) if c.is_ascii_lowercase() || c == '_');
    starts
        && name.len() <= 63
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !name.starts_with("pg_")
        && name != "public"
        && name != "information_schema"
}

/// The limits a transaction runs under, in the units PostgreSQL takes (milliseconds): the same limit as the
/// client-side timeout, so a statement the server is still working on when this engine gives up can't hold its
/// locks for long.
///
/// `SET LOCAL` rather than a session-level `SET` after connecting: a transaction-mode pooler (PgBouncer, the pooled
/// endpoints of Supabase and Neon) does not reset a server connection when it hands it to another client, so a
/// session-level limit would stick to a shared connection and abort someone else's statement. `SET LOCAL` ends with
/// the transaction it guards, and a pooler keeps a transaction on one connection by definition.
fn transaction_limits(limit: Duration) -> String {
    let ms = limit.as_millis().max(1);
    format!("SET LOCAL statement_timeout = {ms}; SET LOCAL lock_timeout = {ms}")
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

/// The server's own message is added, since the driver's says only "db error". A statement the server stopped for
/// its `statement_timeout` or `lock_timeout`, or for our cancel request, is a timeout like the client-side one.
fn io_error(e: tokio_postgres::Error) -> io::Error {
    let timeouts = [SqlState::QUERY_CANCELED, SqlState::LOCK_NOT_AVAILABLE];
    let kind = match e.code() {
        Some(code) if timeouts.contains(code) => io::ErrorKind::TimedOut,
        _ if e.is_closed() => io::ErrorKind::ConnectionAborted,
        _ => io::ErrorKind::Other,
    };
    match e.as_db_error() {
        Some(db) => io::Error::new(kind, format!("postgres: {}: {}", db.code().code(), db.message())),
        None => io::Error::new(kind, format!("postgres: {e}")),
    }
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

async fn load_session(client: &Client, sql: &Sql) -> io::Result<Option<SessionData>> {
    let row = client.query_opt(sql.load_session.as_str(), &[]).await.map_err(io_error)?;
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

async fn save_session(client: &mut Client, sql: &Sql, limits: &str, s: &SessionData) -> io::Result<()> {
    let (address, port) = match s.addr {
        Some(addr) => (Some(addr.ip().to_string()), Some(i32::from(addr.port()))),
        None => (None, None),
    };
    let date = now();
    // `ToSql` is implemented for `&[u8]`, not `[u8]`, so the slice goes in behind its own reference.
    let key = &s.auth_key[..];
    let tx = client.transaction().await.map_err(io_error)?;
    tx.batch_execute(limits).await.map_err(io_error)?;
    tx.execute(sql.clear_sessions.as_str(), &[]).await.map_err(io_error)?;
    tx.execute(
        sql.save_session.as_str(),
        &[&s.dc_id, &address, &port, &s.api_id, &s.test_mode, &key, &date, &s.user_id, &s.is_bot],
    )
    .await
    .map_err(io_error)?;
    tx.commit().await.map_err(io_error)
}

async fn load_state(client: &Client, sql: &Sql) -> io::Result<Option<StoredState>> {
    let saved = client.query_opt(sql.load_owner.as_str(), &[]).await.map_err(io_error)?;
    // This schema is ours alone and `save_state` writes both tables in one transaction, so a state without its
    // owner can't be there.
    let Some(row) = saved else { return Ok(None) };
    let salt = Salt { value: column(&row, "salt")?, time_offset: column(&row, "time_offset")? };
    let owner = column::<i64>(&row, "owner")?;
    let rows = client.query(sql.load_update_state.as_str(), &[]).await.map_err(io_error)?;
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

async fn save_state(client: &mut Client, sql: &Sql, limits: &str, state: &StoredState) -> io::Result<()> {
    let tx = client.transaction().await.map_err(io_error)?;
    tx.batch_execute(limits).await.map_err(io_error)?;
    tx.execute(sql.save_owner.as_str(), &[&state.owner, &state.salt.value, &state.salt.time_offset])
        .await
        .map_err(io_error)?;
    tx.execute(sql.clear_update_state.as_str(), &[]).await.map_err(io_error)?;
    if let Some(u) = state.update.as_ref().filter(|u| u.date != 0) {
        tx.execute(sql.save_update.as_str(), &[&u.pts, &u.qts, &u.date, &u.seq]).await.map_err(io_error)?;
        let channel = tx.prepare(sql.save_channel.as_str()).await.map_err(io_error)?;
        for (id, pts) in &u.channels {
            tx.execute(&channel, &[&get_channel_id(*id), pts]).await.map_err(io_error)?;
        }
    }
    tx.commit().await.map_err(io_error)
}

async fn update_peers(client: &mut Client, sql: &Sql, limits: &str, peers: &[PeerRecord]) -> io::Result<()> {
    if peers.is_empty() {
        return Ok(());
    }
    let tx = client.transaction().await.map_err(io_error)?;
    tx.batch_execute(limits).await.map_err(io_error)?;
    let upsert = tx.prepare(sql.upsert_peer.as_str()).await.map_err(io_error)?;
    let forget = tx.prepare(sql.forget_usernames.as_str()).await.map_err(io_error)?;
    let add = tx.prepare(sql.add_username.as_str()).await.map_err(io_error)?;
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

async fn get_peer(client: &Client, sql: &Sql, id: i64) -> io::Result<Option<PeerRecord>> {
    let Some(row) = client.query_opt(sql.get_peer.as_str(), &[&id]).await.map_err(io_error)? else {
        return Ok(None);
    };
    let mut p = PeerRecord::new(column(&row, "id")?, column::<Option<i64>>(&row, "access_hash")?.unwrap_or(0));
    (p.phone_number, p.updated) = (column(&row, "phone_number")?, column(&row, "last_update_on")?);
    let names = client.query(sql.get_peer_usernames.as_str(), &[&id]).await.map_err(io_error)?;
    p.usernames = names.iter().map(|row| column::<String>(row, "username")).collect::<io::Result<_>>()?;
    Ok(Some(p))
}

// The most recently updated claim wins, the larger id on a tie, as in every engine.
async fn get_peer_where(client: &Client, sql: &Sql, query: &str, key: &str) -> io::Result<Option<PeerRecord>> {
    match client.query_opt(query, &[&key]).await.map_err(io_error)? {
        Some(row) => get_peer(client, sql, column(&row, "id")?).await,
        None => Ok(None),
    }
}

async fn export_peers(client: &Client, sql: &Sql, limit: Option<usize>) -> io::Result<Vec<PeerRecord>> {
    let limit = limit.map(|n| i64::try_from(n).unwrap_or(i64::MAX));
    let rows = client.query(sql.export_ids.as_str(), &[&limit]).await.map_err(io_error)?;
    let ids: Vec<i64> = rows.iter().map(|row| column(row, "id")).collect::<io::Result<_>>()?;
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    // One query for the peers and one for their usernames, rather than a round trip for each peer.
    let peers = client.query(sql.export_peers.as_str(), &[&ids]).await.map_err(io_error)?;
    let names = client.query(sql.export_usernames.as_str(), &[&ids]).await.map_err(io_error)?;
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

async fn clear_peers(client: &Client, sql: &Sql) -> io::Result<()> {
    client.batch_execute(sql.clear_peers.as_str()).await.map_err(io_error)
}

async fn delete(client: &Client, sql: &Sql) -> io::Result<()> {
    client.batch_execute(sql.delete_all.as_str()).await.map_err(io_error)
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
            // The next open checks the schema again: it may have been dropped while the engine was closed.
            self.prepared.store(false, Ordering::Release);
            Ok(())
        })
    }

    fn delete(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let client = self.open_conn().await?;
            let result = limited(self.timeout, delete(&client, &self.sql)).await;
            self.keep_after(client, result).await
        })
    }

    fn load_session(&self) -> StorageFuture<'_, Option<SessionData>> {
        Box::pin(async move {
            let client = self.open_conn().await?;
            let result = limited(self.timeout, load_session(&client, &self.sql)).await;
            self.keep_after(client, result).await
        })
    }

    fn save_session<'a>(&'a self, session: &'a SessionData) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let mut client = self.open_conn().await?;
            let result = limited(self.timeout, save_session(&mut client, &self.sql, &self.limits, session)).await;
            self.keep_after(client, result).await
        })
    }

    fn load_state(&self) -> StorageFuture<'_, Option<StoredState>> {
        Box::pin(async move {
            let client = self.open_conn().await?;
            let result = limited(self.timeout, load_state(&client, &self.sql)).await;
            self.keep_after(client, result).await
        })
    }

    fn save_state<'a>(&'a self, state: &'a StoredState) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let mut client = self.open_conn().await?;
            let result = limited(self.timeout, save_state(&mut client, &self.sql, &self.limits, state)).await;
            self.keep_after(client, result).await
        })
    }

    fn update_peers<'a>(&'a self, peers: &'a [PeerRecord]) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let mut client = self.open_conn().await?;
            let result = limited(self.timeout, update_peers(&mut client, &self.sql, &self.limits, peers)).await;
            self.keep_after(client, result).await
        })
    }

    fn get_peer_by_id(&self, id: i64) -> StorageFuture<'_, Option<PeerRecord>> {
        Box::pin(async move {
            let client = self.open_conn().await?;
            let result = limited(self.timeout, get_peer(&client, &self.sql, id)).await;
            self.keep_after(client, result).await
        })
    }

    fn get_peer_by_username<'a>(&'a self, username: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        Box::pin(async move {
            // A NUL byte can't be in a PostgreSQL text value, and no stored name has one.
            if !storable(username) {
                return Ok(None);
            }
            let client = self.open_conn().await?;
            let query = self.sql.peer_by_username.as_str();
            let result = limited(self.timeout, get_peer_where(&client, &self.sql, query, username)).await;
            self.keep_after(client, result).await
        })
    }

    fn get_peer_by_phone_number<'a>(&'a self, phone_number: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        Box::pin(async move {
            if !storable(phone_number) {
                return Ok(None);
            }
            let client = self.open_conn().await?;
            let query = self.sql.peer_by_phone_number.as_str();
            let result = limited(self.timeout, get_peer_where(&client, &self.sql, query, phone_number)).await;
            self.keep_after(client, result).await
        })
    }

    fn export_peers(&self, limit: Option<usize>) -> StorageFuture<'_, Vec<PeerRecord>> {
        Box::pin(async move {
            let client = self.open_conn().await?;
            let result = limited(self.timeout, export_peers(&client, &self.sql, limit)).await;
            self.keep_after(client, result).await
        })
    }

    fn clear_peers(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let client = self.open_conn().await?;
            let result = limited(self.timeout, clear_peers(&client, &self.sql)).await;
            self.keep_after(client, result).await
        })
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::time::{Duration, Instant};

    use tokio_postgres::{Config, NoTls};

    use super::{PostgresStorage, Sql, valid_name};
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
        for name in ["", "Bot", "a-b", "a\"b", "pg_x", "9x", "a b", "public", "information_schema"] {
            assert!(!valid_name(name), "{name}");
        }
        assert!(valid_name(&"a".repeat(63)));
        assert!(!valid_name(&"a".repeat(64)));
    }

    /// A transaction-mode pooler may run the next statement on another connection, so no statement may depend on
    /// a session setting: each one names the schema itself.
    #[test]
    fn every_statement_names_the_schema() {
        let sql = Sql::new("vaegram_test");
        let prefix = "\"vaegram_test\".";
        let ident = |b: &u8| b.is_ascii_alphanumeric() || *b == b'_';
        for statement in sql.statements() {
            for table in ["sessions", "peers", "usernames", "update_state", "version", "vaegram"] {
                for (at, _) in statement.match_indices(table) {
                    let end = at + table.len();
                    let before = at.checked_sub(1).and_then(|i| statement.as_bytes().get(i));
                    if before.is_some_and(ident) || statement.as_bytes().get(end).is_some_and(ident) {
                        // Part of a longer name, such as an index or the schema itself.
                        continue;
                    }
                    assert!(statement[..at].ends_with(prefix), "{table} is unqualified in {statement}");
                }
            }
        }
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

    /// A server that accepts the connection and never answers the login: only the limit can end it, and it covers
    /// the login, not just the TCP handshake.
    #[tokio::test]
    async fn a_server_that_never_answers_times_out() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let accepted = tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((socket, _)) = listener.accept().await {
                held.push(socket);
            }
        });
        let mut config = Config::new();
        config.host("127.0.0.1").port(port).user("vaegram").dbname("vaegram_test");
        let limit = Duration::from_millis(200);
        let storage = PostgresStorage::new(config, NoTls, "vaegram_login_test").timeout(limit);
        let started = Instant::now();
        let error = storage.open().await.unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut, "{error}");
        assert!(started.elapsed() < Duration::from_secs(5), "gave up after {:?}", started.elapsed());
        accepted.abort();
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
        // A regression would otherwise hang CI rather than fail it.
        tokio::time::timeout(Duration::from_secs(60), blocked_statement_times_out()).await.unwrap();
    }

    async fn blocked_statement_times_out() {
        let Some(url) = url() else { return };
        let mut config = test_config(&url).unwrap();
        let locker_config = config.clone();
        config.application_name("vaegram-timeout-test");
        let limit = Duration::from_millis(500);
        let storage = PostgresStorage::new(config, NoTls, "vaegram_timeout_test").timeout(limit);
        storage.open().await.unwrap();

        // A second connection holds the table, so every save waits for it.
        let (locker, connection) = locker_config.connect(NoTls).await.unwrap();
        tokio::spawn(connection);
        locker
            .batch_execute("BEGIN; LOCK TABLE \"vaegram_timeout_test\".sessions IN ACCESS EXCLUSIVE MODE")
            .await
            .unwrap();

        for attempt in 1..=3 {
            let started = Instant::now();
            let error = storage.save_session(&SessionData::new(2, [1; 256])).await.unwrap_err();
            assert!(
                started.elapsed() < limit * 4,
                "attempt {attempt} took {:?}, not about the {limit:?} limit",
                started.elapsed()
            );
            assert_eq!(error.kind(), std::io::ErrorKind::TimedOut, "attempt {attempt}: {error}");
            // The connection that ran out of time is not parked on the server: the next save opens a new one, and
            // the old backend is gone (or going), not stuck until the statement it was running answers.
            let left: i64 = locker
                .query_one(
                    "SELECT count(*) FROM pg_stat_activity \
                     WHERE datname = current_database() AND application_name = 'vaegram-timeout-test'",
                    &[],
                )
                .await
                .unwrap()
                .get(0);
            assert!(left <= 1, "attempt {attempt} left {left} backends");
        }

        // With the table free again the next save goes through, on a new connection, without waiting.
        locker.batch_execute("ROLLBACK").await.unwrap();
        let started = Instant::now();
        storage.save_session(&SessionData::new(2, [1; 256])).await.unwrap();
        assert!(started.elapsed() < limit * 4, "the save after the lock was released took {:?}", started.elapsed());
    }
}
