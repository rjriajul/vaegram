use std::collections::HashMap;
use std::fmt;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, ErrorCode, OptionalExtension, Transaction, params};

use super::engine::{PeerRecord, Storage, StorageFuture, StoredState};
use super::{SessionData, UpdateState, create_private};
use crate::session::Salt;
use crate::session::internals::data_center;
use crate::utils::{CHANNEL_OFFSET, get_channel_id};

const VERSION: i64 = 8;

// Schema version 8 of the session database the Python MTProto libraries keep, so their files open here and
// files written here open there. `vaegram` holds what that layout has no place for.
const SCHEMA: &str = "
CREATE TABLE sessions
(
    dc_id          INTEGER PRIMARY KEY,
    server_address TEXT,
    port           INTEGER,
    api_id         INTEGER,
    test_mode      INTEGER,
    auth_key       BLOB,
    date           INTEGER NOT NULL,
    user_id        INTEGER,
    is_bot         INTEGER
);

CREATE TABLE peers
(
    id             INTEGER PRIMARY KEY,
    access_hash    INTEGER,
    type           INTEGER NOT NULL,
    phone_number   TEXT,
    last_update_on INTEGER NOT NULL DEFAULT (CAST(STRFTIME('%s', 'now') AS INTEGER))
);

CREATE TABLE usernames
(
    id       INTEGER,
    username TEXT,
    FOREIGN KEY (id) REFERENCES peers(id)
);

CREATE TABLE update_state
(
    id   INTEGER PRIMARY KEY,
    pts  INTEGER,
    qts  INTEGER,
    date INTEGER,
    seq  INTEGER
);

CREATE TABLE version
(
    number INTEGER PRIMARY KEY
);

CREATE INDEX idx_peers_id ON peers (id);
CREATE INDEX idx_peers_phone_number ON peers (phone_number);
CREATE INDEX idx_usernames_id ON usernames (id);
CREATE INDEX idx_usernames_username ON usernames (username);

CREATE TRIGGER trg_peers_last_update_on
    AFTER UPDATE
    ON peers
BEGIN
    UPDATE peers
    SET last_update_on = CAST(STRFTIME('%s', 'now') AS INTEGER)
    WHERE id = NEW.id;
END;
";

const USERNAMES_SCHEMA: &str = "
CREATE TABLE usernames
(
    id       INTEGER,
    username TEXT,
    FOREIGN KEY (id) REFERENCES peers(id)
);

CREATE INDEX idx_usernames_username ON usernames (username);
";

const UPDATE_STATE_SCHEMA: &str = "
CREATE TABLE update_state
(
    id   INTEGER PRIMARY KEY,
    pts  INTEGER,
    qts  INTEGER,
    date INTEGER,
    seq  INTEGER
);
";

const VAEGRAM_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS vaegram
(
    id          INTEGER PRIMARY KEY CHECK (id = 0),
    owner       INTEGER NOT NULL,
    salt        INTEGER NOT NULL,
    time_offset INTEGER NOT NULL
);
";

const NEWER: &str = "this session database needs a newer vaegram";
const NOT_A_SESSION: &str = "not a session database";

/// Keeps everything in a SQLite database with the session layout the Python MTProto libraries use (schema
/// version 8), so their `.session` files open here: older versions are migrated in place, so keep a copy first.
///
/// The file is created private to the user (0600 on Unix) before SQLite opens it, and runs in WAL mode.
///
/// Usernames are matched in lowercase, as written by this library and the Python ones.
pub struct SqliteStorage {
    path: PathBuf,
    conn: Mutex<Option<Connection>>,
}

impl SqliteStorage {
    pub fn new(path: impl Into<PathBuf>) -> SqliteStorage {
        SqliteStorage { path: path.into(), conn: Mutex::new(None) }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn lock(&self) -> MutexGuard<'_, Option<Connection>> {
        self.conn.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn named(&self, e: io::Error) -> io::Error {
        io::Error::new(e.kind(), format!("{}: {e}", self.path.display()))
    }

    /// Runs `f` on the open database, opening and migrating it first if it isn't.
    fn with<T>(&self, f: impl FnOnce(&mut Connection) -> io::Result<T>) -> io::Result<T> {
        let mut guard = self.lock();
        let mut conn = match guard.take() {
            Some(conn) => conn,
            None => self.connect().map_err(|e| self.named(e))?,
        };
        let result = f(&mut conn).map_err(|e| self.named(e));
        *guard = Some(conn);
        result
    }

    fn connect(&self) -> io::Result<Connection> {
        let fresh = !self.path.exists();
        if fresh {
            // SQLite would create it with the default mode, readable by everyone on most systems.
            create_private(&self.path)?;
        }
        let mut conn = Connection::open(&self.path).map_err(sql)?;
        conn.busy_timeout(Duration::from_secs(5)).map_err(sql)?;
        // Only reads until the file is known to be a session database, and the migration runs in one transaction:
        // anything else is left byte for byte.
        let version = version(&conn)?;
        migrate(&mut conn, version)?;
        #[cfg(unix)]
        if !fresh {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600))?;
        }
        conn.query_row("PRAGMA journal_mode=WAL", [], |r| r.get::<_, String>(0)).map_err(sql)?;
        conn.execute_batch("PRAGMA synchronous=NORMAL").map_err(sql)?;
        Ok(conn)
    }
}

fn sql(e: rusqlite::Error) -> io::Error {
    let kind = match e.sqlite_error_code() {
        Some(ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt) => io::ErrorKind::InvalidData,
        _ => io::ErrorKind::Other,
    };
    io::Error::new(kind, e.to_string())
}

fn refuse(why: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, why.to_owned())
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// The schema version of a session database, `None` for an empty one; anything else is refused.
fn version(conn: &Connection) -> io::Result<Option<i64>> {
    let tables: i64 = conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get(0)).map_err(sql)?;
    if tables == 0 {
        return Ok(None);
    }
    let version = conn.query_row("SELECT number FROM version", [], |r| r.get::<_, i64>(0));
    match version {
        Ok(v @ 1..=VERSION) => Ok(Some(v)),
        Ok(v) if v > VERSION => Err(refuse(NEWER)),
        // A missing table or column fails to compile, reported with or without where in the SQL.
        Ok(_) | Err(rusqlite::Error::QueryReturnedNoRows | rusqlite::Error::SqlInputError { .. }) => {
            Err(refuse(NOT_A_SESSION))
        }
        Err(e) if e.sqlite_error_code() == Some(ErrorCode::Unknown) => Err(refuse(NOT_A_SESSION)),
        Err(e) => Err(sql(e)),
    }
}

fn migrate(conn: &mut Connection, version: Option<i64>) -> io::Result<()> {
    let tx = conn.transaction().map_err(sql)?;
    match version {
        None => {
            // auto_vacuum only takes effect before the first table exists.
            tx.execute_batch("PRAGMA auto_vacuum=INCREMENTAL").map_err(sql)?;
            tx.execute_batch(SCHEMA).map_err(sql)?;
            tx.execute("INSERT INTO version VALUES (?)", [VERSION]).map_err(sql)?;
            tx.execute(
                "INSERT INTO sessions (dc_id, server_address, port, date) VALUES (2, '149.154.167.51', 443, 0)",
                [],
            )
            .map_err(sql)?;
        }
        Some(from) if from < VERSION => upgrade(&tx, from)?,
        Some(_) => {}
    }
    tx.execute_batch(VAEGRAM_SCHEMA).map_err(sql)?;
    tx.commit().map_err(sql)
}

fn upgrade(tx: &Transaction<'_>, from: i64) -> io::Result<()> {
    let step = |sql_text: &str| tx.execute_batch(sql_text).map_err(sql);
    if from <= 1 {
        step("DELETE FROM peers;")?;
    }
    if from <= 2 {
        step("ALTER TABLE sessions ADD api_id INTEGER;")?;
    }
    if from <= 3 {
        step(USERNAMES_SCHEMA)?;
    }
    if from <= 4 {
        step(UPDATE_STATE_SCHEMA)?;
    }
    if from <= 5 {
        step("CREATE INDEX idx_usernames_id ON usernames (id);")?;
    }
    if from <= 6 {
        step("ALTER TABLE sessions ADD server_address TEXT; ALTER TABLE sessions ADD port INTEGER;")?;
    }
    if from <= 7 {
        let home: Option<(i32, bool)> = tx
            .query_row("SELECT dc_id, test_mode FROM sessions LIMIT 1", [], |r| {
                Ok((r.get(0)?, r.get::<_, Option<bool>>(1)?.unwrap_or(false)))
            })
            .optional()
            .map_err(sql)?;
        if let Some((ip, test)) = home.and_then(|(dc_id, test)| Some((data_center::address(dc_id, test, false)?, test)))
        {
            // The same address and port the other libraries fill in, test DCs on port 80.
            let port = if test { 80 } else { ip.port() };
            tx.execute("UPDATE sessions SET server_address = ?, port = ?", params![ip.ip().to_string(), port])
                .map_err(sql)?;
        }
    }
    tx.execute("UPDATE version SET number = ?", [VERSION]).map_err(sql)?;
    Ok(())
}

fn load_session(conn: &Connection) -> rusqlite::Result<Option<SessionData>> {
    let row = conn
        .query_row(
            "SELECT dc_id, server_address, port, api_id, test_mode, auth_key, user_id, is_bot FROM sessions LIMIT 1",
            [],
            |r| {
                let key: Option<Vec<u8>> = r.get(5)?;
                let Some(auth_key) = key.and_then(|k| <[u8; 256]>::try_from(k).ok()) else { return Ok(None) };
                let mut s = SessionData::new(r.get(0)?, auth_key);
                let address: Option<String> = r.get(1)?;
                let port: Option<u16> = r.get(2)?;
                let ip = address.and_then(|a| a.parse::<IpAddr>().ok());
                s.addr = ip.zip(port.filter(|&p| p != 0)).map(|(ip, port)| SocketAddr::new(ip, port));
                s.api_id = r.get::<_, Option<i32>>(3)?.unwrap_or(0);
                s.test_mode = r.get::<_, Option<bool>>(4)?.unwrap_or(false);
                s.user_id = r.get::<_, Option<i64>>(6)?.unwrap_or(0);
                s.is_bot = r.get::<_, Option<bool>>(7)?.unwrap_or(false);
                Ok(Some(s))
            },
        )
        .optional()?;
    Ok(row.flatten())
}

fn save_session(conn: &mut Connection, s: &SessionData) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM sessions", [])?;
    tx.execute(
        "INSERT INTO sessions (dc_id, server_address, port, api_id, test_mode, auth_key, date, user_id, is_bot) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            s.dc_id,
            s.addr.map(|a| a.ip().to_string()),
            s.addr.map(|a| a.port()),
            s.api_id,
            s.test_mode,
            &s.auth_key[..],
            now(),
            s.user_id,
            s.is_bot,
        ],
    )?;
    tx.commit()
}

fn load_state(conn: &Connection) -> rusqlite::Result<Option<StoredState>> {
    let saved: Option<(i64, Salt)> = conn
        .query_row("SELECT owner, salt, time_offset FROM vaegram WHERE id = 0", [], |r| {
            Ok((r.get(0)?, Salt { value: r.get(1)?, time_offset: r.get(2)? }))
        })
        .optional()?;
    let mut update: Option<UpdateState> = None;
    let mut channels = HashMap::new();
    let mut stmt = conn.prepare("SELECT id, pts, qts, date, seq FROM update_state")?;
    let rows = stmt.query_map([], |r| {
        let n = |i: usize| r.get::<_, Option<i32>>(i).map(Option::unwrap_or_default);
        Ok((r.get::<_, i64>(0)?, n(1)?, n(2)?, n(3)?, n(4)?))
    })?;
    for row in rows {
        let (id, pts, qts, date, seq) = row?;
        if id == 0 {
            update = Some(UpdateState { pts, qts, date, seq, channels: HashMap::new() });
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
            let owner = load_session(conn)?.map_or(0, |s| s.user_id);
            Some(StoredState { owner, salt: Salt::default(), update })
        }
        None => None,
    })
}

fn save_state(conn: &mut Connection, state: &StoredState) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "REPLACE INTO vaegram (id, owner, salt, time_offset) VALUES (0, ?, ?, ?)",
        params![state.owner, state.salt.value, state.salt.time_offset],
    )?;
    tx.execute("DELETE FROM update_state", [])?;
    if let Some(u) = state.update.as_ref().filter(|u| u.date != 0) {
        tx.execute("INSERT INTO update_state VALUES (0, ?, ?, ?, ?)", params![u.pts, u.qts, u.date, u.seq])?;
        let mut channel = tx.prepare("INSERT INTO update_state (id, pts) VALUES (?, ?)")?;
        for (id, pts) in &u.channels {
            channel.execute(params![get_channel_id(*id), pts])?;
        }
        drop(channel);
    }
    tx.commit()
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

fn update_peers(conn: &mut Connection, peers: &[PeerRecord]) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    {
        let mut peer = tx.prepare(
            "REPLACE INTO peers (id, access_hash, type, phone_number, last_update_on) VALUES (?, ?, ?, ?, ?)",
        )?;
        let mut forget = tx.prepare("DELETE FROM usernames WHERE id = ?")?;
        let mut username = tx.prepare("INSERT INTO usernames (id, username) VALUES (?, ?)")?;
        for p in peers {
            peer.execute(params![p.id, p.access_hash, peer_type(p.id), p.phone_number, p.updated])?;
            forget.execute([p.id])?;
            for u in &p.usernames {
                username.execute(params![p.id, u])?;
            }
        }
    }
    tx.commit()
}

fn get_peer(conn: &Connection, id: i64) -> rusqlite::Result<Option<PeerRecord>> {
    let Some(mut p) = conn
        .query_row("SELECT id, access_hash, phone_number, last_update_on FROM peers WHERE id = ?", [id], |r| {
            let mut p = PeerRecord::new(r.get(0)?, r.get::<_, Option<i64>>(1)?.unwrap_or(0));
            (p.phone_number, p.updated) = (r.get(2)?, r.get(3)?);
            Ok(p)
        })
        .optional()?
    else {
        return Ok(None);
    };
    let mut usernames = conn.prepare("SELECT username FROM usernames WHERE id = ? ORDER BY rowid")?;
    p.usernames = usernames.query_map([id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    Ok(Some(p))
}

// The most recently updated claim wins, the larger id on a tie, as in every engine.
fn get_peer_where(conn: &Connection, query: &str, key: &str) -> rusqlite::Result<Option<PeerRecord>> {
    match conn.query_row(query, [key], |r| r.get::<_, i64>(0)).optional()? {
        Some(id) => get_peer(conn, id),
        None => Ok(None),
    }
}

fn export_peers(conn: &Connection, limit: Option<usize>) -> rusqlite::Result<Vec<PeerRecord>> {
    let limit = limit.map_or(-1, |n| i64::try_from(n).unwrap_or(i64::MAX));
    let mut ids = conn.prepare("SELECT id FROM peers ORDER BY last_update_on DESC, id ASC LIMIT ?")?;
    let ids: Vec<i64> = ids.query_map([limit], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    let mut peers = Vec::with_capacity(ids.len());
    for id in ids {
        peers.extend(get_peer(conn, id)?);
    }
    Ok(peers)
}

impl fmt::Debug for SqliteStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqliteStorage").field("path", &self.path).finish_non_exhaustive()
    }
}

impl Storage for SqliteStorage {
    fn open(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move { self.with(|_| Ok(())) })
    }

    fn close(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let Some(conn) = self.lock().take() else { return Ok(()) };
            conn.close().map_err(|(_, e)| self.named(sql(e)))
        })
    }

    fn delete(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let mut guard = self.lock();
            if let Some(conn) = guard.take() {
                conn.close().map_err(|(_, e)| self.named(sql(e)))?;
            }
            for suffix in ["", "-wal", "-shm"] {
                let mut path = self.path.as_os_str().to_owned();
                path.push(suffix);
                super::delete(Path::new(&path)).map_err(|e| self.named(e))?;
            }
            Ok(())
        })
    }

    fn load_session(&self) -> StorageFuture<'_, Option<SessionData>> {
        Box::pin(async move { self.with(|conn| load_session(conn).map_err(sql)) })
    }

    fn save_session<'a>(&'a self, session: &'a SessionData) -> StorageFuture<'a, ()> {
        Box::pin(async move { self.with(|conn| save_session(conn, session).map_err(sql)) })
    }

    fn load_state(&self) -> StorageFuture<'_, Option<StoredState>> {
        Box::pin(async move { self.with(|conn| load_state(conn).map_err(sql)) })
    }

    fn save_state<'a>(&'a self, state: &'a StoredState) -> StorageFuture<'a, ()> {
        Box::pin(async move { self.with(|conn| save_state(conn, state).map_err(sql)) })
    }

    fn update_peers<'a>(&'a self, peers: &'a [PeerRecord]) -> StorageFuture<'a, ()> {
        Box::pin(async move { self.with(|conn| update_peers(conn, peers).map_err(sql)) })
    }

    fn get_peer_by_id(&self, id: i64) -> StorageFuture<'_, Option<PeerRecord>> {
        Box::pin(async move { self.with(|conn| get_peer(conn, id).map_err(sql)) })
    }

    fn get_peer_by_username<'a>(&'a self, username: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        Box::pin(async move {
            let query = "SELECT p.id FROM peers p JOIN usernames u ON p.id = u.id WHERE u.username = ? \
                         ORDER BY p.last_update_on DESC, p.id DESC LIMIT 1";
            self.with(|conn| get_peer_where(conn, query, username).map_err(sql))
        })
    }

    fn get_peer_by_phone_number<'a>(&'a self, phone_number: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        Box::pin(async move {
            let query = "SELECT id FROM peers WHERE phone_number = ? ORDER BY last_update_on DESC, id DESC LIMIT 1";
            self.with(|conn| get_peer_where(conn, query, phone_number).map_err(sql))
        })
    }

    fn export_peers(&self, limit: Option<usize>) -> StorageFuture<'_, Vec<PeerRecord>> {
        Box::pin(async move { self.with(|conn| export_peers(conn, limit).map_err(sql)) })
    }

    fn clear_peers(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            self.with(|conn| conn.execute_batch("DELETE FROM usernames; DELETE FROM peers;").map_err(sql))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use rusqlite::Connection;

    use super::{SCHEMA, SqliteStorage, VERSION};
    use crate::session::Salt;
    use crate::session::internals::data_center;
    use crate::storage::{PeerRecord, SessionData, Storage, StoredState, UpdateState, conformance};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vaegram-sqlite-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn key() -> Vec<u8> {
        (0..=255).collect()
    }

    #[tokio::test]
    async fn conformance() {
        let dir = temp_dir("conformance");
        let storage = SqliteStorage::new(dir.join("bot.session"));
        conformance::check(&storage).await;
        let left = std::fs::read_dir(&dir).unwrap().count();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(left, 0, "delete leaves no file behind");
    }

    #[tokio::test]
    async fn opens_a_v8_file_built_by_sql() {
        let dir = temp_dir("v8");
        let path = dir.join("other.session");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn.execute("INSERT INTO version VALUES (?)", [VERSION]).unwrap();
        conn.execute(
            "INSERT INTO sessions VALUES (4, '149.154.167.91', 443, 12345, 0, ?, 1700000000, 777000, 1)",
            [key()],
        )
        .unwrap();
        conn.execute("INSERT INTO peers VALUES (100, 5, 'user', '15550001', 10)", []).unwrap();
        conn.execute("INSERT INTO peers VALUES (-1000000001234, 6, 'supergroup', NULL, 20)", []).unwrap();
        conn.execute("INSERT INTO usernames VALUES (-1000000001234, 'news')", []).unwrap();
        conn.execute("INSERT INTO update_state VALUES (0, 10, 2, 1700000000, 5)", []).unwrap();
        conn.execute("INSERT INTO update_state VALUES (-1000000001234, 99, NULL, NULL, NULL)", []).unwrap();
        drop(conn);

        let storage = SqliteStorage::new(&path);
        let session = storage.load_session().await.unwrap().unwrap();
        let state = storage.load_state().await.unwrap();
        let by_username = storage.get_peer_by_username("news").await.unwrap();
        let by_phone = storage.get_peer_by_phone_number("15550001").await.unwrap().map(|p| p.id);
        storage.close().await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);

        let fields = (session.dc_id, session.addr, session.api_id, session.user_id, session.is_bot, session.test_mode);
        assert_eq!(fields, (4, Some("149.154.167.91:443".parse().unwrap()), 12345, 777000, true, false));
        assert_eq!(session.auth_key.to_vec(), key());
        let update =
            UpdateState { pts: 10, qts: 2, date: 1_700_000_000, seq: 5, channels: HashMap::from([(1234, 99)]) };
        assert_eq!(state, Some(StoredState::new(777000, Salt::default(), Some(update))));
        let mut channel = PeerRecord::new(-1_000_000_001_234, 6);
        (channel.usernames, channel.updated) = (vec!["news".into()], 20);
        assert_eq!(by_username, Some(channel));
        assert_eq!(by_phone, Some(100));
    }

    #[tokio::test]
    async fn migrates_a_v3_file() {
        let dir = temp_dir("v3");
        let path = dir.join("old.session");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (dc_id INTEGER PRIMARY KEY, api_id INTEGER, test_mode INTEGER, auth_key BLOB, \
                 date INTEGER NOT NULL, user_id INTEGER, is_bot INTEGER);
             CREATE TABLE peers (id INTEGER PRIMARY KEY, access_hash INTEGER, type INTEGER NOT NULL, username TEXT, \
                 phone_number TEXT, last_update_on INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE version (number INTEGER PRIMARY KEY);
             INSERT INTO version VALUES (3);",
        )
        .unwrap();
        conn.execute("INSERT INTO sessions VALUES (2, 12345, 0, ?, 0, 42, 0)", [key()]).unwrap();
        conn.execute("INSERT INTO peers VALUES (100, 5, 'user', 'alice', NULL, 10)", []).unwrap();
        drop(conn);

        let storage = SqliteStorage::new(&path);
        let session = storage.load_session().await.unwrap().unwrap();
        let peer = storage.get_peer_by_id(100).await.unwrap();
        storage.update_peers(&[PeerRecord::new(200, 7)]).await.unwrap();
        storage.close().await.unwrap();
        let conn = Connection::open(&path).unwrap();
        let version: i64 = conn.query_row("SELECT number FROM version", [], |r| r.get(0)).unwrap();
        let tables: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name IN ('usernames', 'update_state', 'vaegram')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        drop(conn);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!((version, tables), (VERSION, 3));
        assert_eq!((session.dc_id, session.api_id, session.user_id), (2, 12345, 42));
        assert_eq!(session.addr, data_center::address(2, false, false));
        assert_eq!(peer.map(|p| (p.id, p.access_hash)), Some((100, 5)));
    }

    #[tokio::test]
    async fn rows_match_the_v8_columns() {
        let dir = temp_dir("rows");
        let path = dir.join("bot.session");
        let storage = SqliteStorage::new(&path);
        let mut session = SessionData::new(2, [7; 256]);
        (session.user_id, session.is_bot, session.addr) = (42, true, Some("149.154.167.51:443".parse().unwrap()));
        storage.save_session(&session).await.unwrap();
        let update = UpdateState { pts: 3, qts: 0, date: 9, seq: 1, channels: HashMap::from([(1234, 8)]) };
        storage.save_state(&StoredState::new(42, Salt { value: 5, time_offset: -1 }, Some(update))).await.unwrap();
        let mut user = PeerRecord::new(100, 5);
        (user.usernames, user.phone_number, user.updated) = (vec!["alice".into()], Some("1555".into()), 30);
        storage.update_peers(&[user, PeerRecord::new(-1_000_000_001_234, 6)]).await.unwrap();
        storage.close().await.unwrap();

        type PeerRow = (i64, i64, String, Option<String>, i64);
        type StateRow = (i64, i64, Option<i64>, Option<i64>, Option<i64>);
        let (session_row, peers, username, states) = {
            let conn = Connection::open(&path).unwrap();
            let session_row: (i64, String, i64, Vec<u8>, i64, i64) = conn
                .query_row("SELECT dc_id, server_address, port, auth_key, user_id, is_bot FROM sessions", [], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
                })
                .unwrap();
            let mut stmt = conn
                .prepare("SELECT id, access_hash, type, phone_number, last_update_on FROM peers ORDER BY id")
                .unwrap();
            let peers: Vec<PeerRow> = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            let username: (i64, String) =
                conn.query_row("SELECT id, username FROM usernames", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
            let mut stmt = conn.prepare("SELECT id, pts, qts, date, seq FROM update_state ORDER BY id").unwrap();
            let states: Vec<StateRow> = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            (session_row, peers, username, states)
        };
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(session_row, (2, "149.154.167.51".into(), 443, vec![7; 256], 42, 1));
        assert_eq!(
            peers,
            [(-1_000_000_001_234, 6, "channel".into(), None, 0), (100, 5, "user".into(), Some("1555".into()), 30),]
        );
        assert_eq!(username, (100, "alice".into()));
        assert_eq!(states, [(-1_000_000_001_234, 8, None, None, None), (0, 3, Some(0), Some(9), Some(1))]);
    }

    #[tokio::test]
    async fn other_files_are_refused_untouched() {
        let dir = temp_dir("foreign");
        let path = dir.join("bot.session");
        let foreign = b"vaegram-session\0\x01\0\0\0 not a database at all, just bytes".repeat(40);
        std::fs::write(&path, &foreign).unwrap();
        let error = SqliteStorage::new(&path).load_session().await.err().unwrap();
        let left = std::fs::read(&path).unwrap();
        let dbs = [
            (
                "newer",
                "CREATE TABLE version (number INTEGER PRIMARY KEY); INSERT INTO version VALUES (9);",
                "newer vaegram",
            ),
            (
                "address-first",
                "CREATE TABLE version (version INTEGER PRIMARY KEY); INSERT INTO version VALUES (7);
                 CREATE TABLE sessions (dc_id INTEGER PRIMARY KEY, server_address TEXT, port INTEGER, auth_key BLOB);
                 CREATE TABLE entities (id INTEGER PRIMARY KEY, hash INTEGER NOT NULL);",
                "not a session database",
            ),
            ("other-app", "CREATE TABLE notes (body TEXT); INSERT INTO notes VALUES ('hi');", "not a session database"),
            (
                "failed-migration",
                "CREATE TABLE version (number INTEGER PRIMARY KEY); INSERT INTO version VALUES (3);
                 CREATE TABLE notes (body TEXT); INSERT INTO notes VALUES ('hi');",
                "no such table: sessions",
            ),
        ];
        let mut refused = Vec::new();
        for (name, schema, why) in dbs {
            let db = dir.join(format!("{name}.session"));
            let conn = Connection::open(&db).unwrap();
            conn.execute_batch(schema).unwrap();
            drop(conn);
            let before = std::fs::read(&db).unwrap();
            let error = SqliteStorage::new(&db).open().await.err().unwrap().to_string();
            let after = std::fs::read(&db).unwrap();
            let sidecars = ["-wal", "-shm", "-journal"].map(|s| dir.join(format!("{name}.session{s}")).exists());
            refused.push((name, error.contains(why), before == after, sidecars));
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData, "{error}");
        assert_eq!(left, foreign);
        for (name, explained, untouched, sidecars) in refused {
            assert!(explained && untouched && sidecars == [false; 3], "{name}: {explained} {untouched} {sidecars:?}");
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn file_is_private_on_unix() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("private");
        let storage = SqliteStorage::new(dir.join("bot.session"));
        storage.save_session(&SessionData::new(2, [1; 256])).await.unwrap();
        let mode = |name: &str| std::fs::metadata(dir.join(name)).unwrap().permissions().mode() & 0o777;
        let modes = (mode("bot.session"), mode("bot.session-wal"));
        storage.close().await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(modes, (0o600, 0o600));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn an_existing_file_is_made_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("existing");
        let path = dir.join("other.session");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn.execute("INSERT INTO version VALUES (?)", [VERSION]).unwrap();
        drop(conn);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let storage = SqliteStorage::new(&path);
        storage.open().await.unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        storage.close().await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(mode, 0o600);
    }
}
