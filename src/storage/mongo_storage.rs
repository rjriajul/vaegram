use std::collections::HashMap;
use std::fmt;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use mongodb::bson::spec::BinarySubtype;
use mongodb::bson::{Binary, Bson, Document, doc};
use mongodb::error::ErrorKind;
use mongodb::{Client, Collection, Cursor, Database, IndexModel};

use super::engine::{PeerRecord, Storage, StorageFuture, StoredState};
use super::{SessionData, UpdateState};
use crate::session::Salt;
use crate::utils::{CHANNEL_OFFSET, get_channel_id};

const VERSION: i64 = 1;

/// Keeps everything in MongoDB, in the collections the Python MTProto libraries use, in the database named after
/// the session unless [`database`](Self::database) says otherwise. Point it at their database to pick up a session
/// they wrote.
///
/// Writes replace documents in place and need no transactions, so a standalone server works. A username passes to
/// the newest remaining peer that lists it when its owner lets it go. Usernames are matched in lowercase, as written
/// by this library and the Python ones.
pub struct MongoStorage {
    uri: String,
    database: String,
    db: Mutex<Option<Database>>,
    // How many more writes succeed before one fails, so tests can stop a save midway.
    #[cfg(test)]
    writes_left: std::sync::atomic::AtomicUsize,
}

impl MongoStorage {
    /// `uri` is a `mongodb://` or `mongodb+srv://` URI; nothing connects until the storage is opened.
    pub fn new(uri: impl Into<String>, name: impl Into<String>) -> MongoStorage {
        MongoStorage {
            uri: uri.into(),
            database: name.into(),
            db: Mutex::new(None),
            #[cfg(test)]
            writes_left: std::sync::atomic::AtomicUsize::new(usize::MAX),
        }
    }

    /// Called before every write of a save, so tests can fail one.
    fn write(&self) -> io::Result<()> {
        #[cfg(test)]
        {
            use std::sync::atomic::Ordering::SeqCst;
            // Tests drive one save at a time, so a plain load and store is enough.
            let left = self.writes_left.load(SeqCst);
            if left == 0 {
                return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "mongodb: injected fault"));
            }
            self.writes_left.store(left - 1, SeqCst);
        }
        Ok(())
    }

    /// Uses `database` instead of the session name.
    #[must_use]
    pub fn database(mut self, database: impl Into<String>) -> MongoStorage {
        self.database = database.into();
        self
    }

    async fn db(&self) -> io::Result<Database> {
        if let Some(db) = self.db.lock().unwrap_or_else(PoisonError::into_inner).clone() {
            return Ok(db);
        }
        let client = Client::with_uri_str(&self.uri).await.map_err(|e| match *e.kind {
            // The parser's message can quote the URI, password included.
            ErrorKind::InvalidArgument { .. } => io::Error::new(io::ErrorKind::InvalidInput, "not a MongoDB URI"),
            _ => io_error(e),
        })?;
        let db = client.database(&self.database);
        let version: Option<Document> = db.collection("version").find_one(doc! { "_id": 0 }).await.map_err(io_error)?;
        if version.as_ref().and_then(|v| int(v, "number")).is_some_and(|v| v > VERSION) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "this MongoDB session needs a newer vaegram"));
        }
        let peers: Collection<Document> = db.collection("peers");
        let usernames: Collection<Document> = db.collection("usernames");
        peers.create_index(IndexModel::builder().keys(doc! { "phone_number": 1 }).build()).await.map_err(io_error)?;
        usernames.create_index(IndexModel::builder().keys(doc! { "peer_id": 1 }).build()).await.map_err(io_error)?;
        peers.create_index(IndexModel::builder().keys(doc! { "usernames": 1 }).build()).await.map_err(io_error)?;
        *self.db.lock().unwrap_or_else(PoisonError::into_inner) = Some(db.clone());
        Ok(db)
    }

    async fn collection(&self, name: &str) -> io::Result<Collection<Document>> {
        Ok(self.db().await?.collection(name))
    }

    async fn load_session(&self) -> io::Result<Option<SessionData>> {
        let found = self.collection("session").await?.find_one(doc! { "_id": 0 }).await.map_err(io_error)?;
        Ok(found.as_ref().and_then(session))
    }

    async fn save_session(&self, s: &SessionData) -> io::Result<()> {
        let db = self.db().await?;
        let fields = session_document(s, now());
        let session: Collection<Document> = db.collection("session");
        session.update_one(doc! { "_id": 0 }, doc! { "$set": fields }).upsert(true).await.map_err(io_error)?;
        let version: Collection<Document> = db.collection("version");
        let number = doc! { "$set": { "number": VERSION } };
        version.update_one(doc! { "_id": 0 }, number).upsert(true).await.map_err(io_error)?;
        Ok(())
    }

    async fn load_state(&self) -> io::Result<Option<StoredState>> {
        let db = self.db().await?;
        let saved = db.collection::<Document>("vaegram").find_one(doc! { "_id": 0 }).await.map_err(io_error)?;
        let rows = all(db.collection("update_state").find(doc! {}).await.map_err(io_error)?).await?;
        let update = update_state(&rows);
        if let Some(saved) = saved {
            let salt =
                Salt { value: int(&saved, "salt").unwrap_or(0), time_offset: int(&saved, "time_offset").unwrap_or(0) };
            return Ok(Some(StoredState { owner: int(&saved, "owner").unwrap_or(0), salt, update }));
        }
        if update.is_none() {
            return Ok(None);
        }
        // Another library's session: its account owns the update state.
        let owner = self.load_session().await?.map_or(0, |s| s.user_id);
        Ok(Some(StoredState { owner, salt: Salt::default(), update }))
    }

    async fn save_state(&self, state: &StoredState) -> io::Result<()> {
        let db = self.db().await?;
        let saved =
            doc! { "$set": { "owner": state.owner, "salt": state.salt.value, "time_offset": state.salt.time_offset } };
        let vaegram: Collection<Document> = db.collection("vaegram");
        self.write()?;
        vaegram.update_one(doc! { "_id": 0 }, saved).upsert(true).await.map_err(io_error)?;
        // Each row is replaced where it is, then the rows the state no longer has go: no moment without a state.
        let states: Collection<Document> = db.collection("update_state");
        let rows = state_rows(state);
        let mut kept = Vec::with_capacity(rows.len());
        for row in rows {
            let id = row.get("_id").cloned().unwrap_or(Bson::Null);
            self.write()?;
            states.replace_one(doc! { "_id": id.clone() }, row).upsert(true).await.map_err(io_error)?;
            kept.push(id);
        }
        self.write()?;
        states.delete_many(doc! { "_id": { "$nin": kept } }).await.map_err(io_error)?;
        Ok(())
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
        let db = self.db().await?;
        let (peer_docs, names): (Collection<Document>, Collection<Document>) =
            (db.collection("peers"), db.collection("usernames"));
        let ids: Vec<i64> = batch.keys().copied().collect();

        // Every name a batch peer holds now or held before.
        let held = all(names.find(doc! { "peer_id": { "$in": ids.clone() } }).await.map_err(io_error)?).await?;
        let mut touched: Vec<String> = held.iter().filter_map(|d| d.get_str("_id").ok().map(str::to_owned)).collect();
        touched.extend(batch.values().flat_map(|p| p.usernames.iter().cloned()));
        touched.sort_unstable();
        touched.dedup();

        // Peers first, so a username never points at a peer that isn't there yet.
        for p in batch.values() {
            self.write()?;
            peer_docs.replace_one(doc! { "_id": p.id }, peer_document(p)).upsert(true).await.map_err(io_error)?;
        }
        for name in &touched {
            let current = names.find_one(doc! { "_id": name.as_str() }).await.map_err(io_error)?;
            let current = current.as_ref().and_then(|d| int(d, "peer_id"));
            let owner = self.newest_holder(&peer_docs, name, current).await?;
            if owner == current {
                continue;
            }
            self.write()?;
            match owner {
                Some(owner) => {
                    let set = doc! { "$set": { "peer_id": owner } };
                    names.update_one(doc! { "_id": name.as_str() }, set).upsert(true).await.map_err(io_error)?;
                }
                None => {
                    names.delete_one(doc! { "_id": name.as_str() }).await.map_err(io_error)?;
                }
            }
        }
        Ok(())
    }

    /// The newest `(updated, id)` peer listing `name`. A peer written by another library lists no names of its
    /// own, so the current holder counts too when it is one of those.
    async fn newest_holder(
        &self,
        peer_docs: &Collection<Document>,
        name: &str,
        current: Option<i64>,
    ) -> io::Result<Option<i64>> {
        let sort = doc! { "last_update_on": -1, "_id": -1 };
        let listed = all(peer_docs.find(doc! { "usernames": name }).sort(sort).limit(1).await.map_err(io_error)?);
        let mut newest =
            listed.await?.first().and_then(|d| Some((int(d, "last_update_on").unwrap_or(0), int(d, "_id")?)));
        if let Some(current) = current {
            let doc = peer_docs.find_one(doc! { "_id": current }).await.map_err(io_error)?;
            if let Some(doc) = doc.filter(|d| d.get_array("usernames").is_err()) {
                newest = newest.max(Some((int(&doc, "last_update_on").unwrap_or(0), current)));
            }
        }
        Ok(newest.map(|(_, id)| id))
    }

    async fn peer_where(&self, filter: Document) -> io::Result<Option<PeerRecord>> {
        let peers = self.collection("peers").await?;
        let sort = doc! { "last_update_on": -1, "_id": -1 };
        let found = all(peers.find(filter).sort(sort).limit(1).await.map_err(io_error)?).await?;
        match found.first() {
            Some(d) => self.record(d).await,
            None => Ok(None),
        }
    }

    async fn record(&self, d: &Document) -> io::Result<Option<PeerRecord>> {
        let Some(mut p) = peer(d) else { return Ok(None) };
        if d.get_array("usernames").is_err() {
            // Peers written elsewhere keep their usernames only in the usernames collection.
            let names = self.collection("usernames").await?;
            let found = all(names.find(doc! { "peer_id": p.id }).sort(doc! { "_id": 1 }).await.map_err(io_error)?);
            p.usernames = found.await?.iter().filter_map(|d| d.get_str("_id").ok().map(str::to_owned)).collect();
        }
        Ok(Some(p))
    }

    /// The peer the usernames collection names, if it still lists the name; a save cut short can leave a name with a
    /// peer that dropped it, and then the newest peer listing it is the owner. After an interrupted batch a name can
    /// also still give an older holder that does list it: stale but valid, until the next batch touching the name.
    async fn get_peer_by_username(&self, username: &str) -> io::Result<Option<PeerRecord>> {
        let names = self.collection("usernames").await?;
        let found = names.find_one(doc! { "_id": username }).await.map_err(io_error)?;
        let Some(id) = found.as_ref().and_then(|d| int(d, "peer_id")) else { return Ok(None) };
        match self.peer_where(doc! { "_id": id }).await? {
            Some(peer) if peer.usernames.iter().any(|n| n == username) => Ok(Some(peer)),
            _ => self.peer_where(doc! { "usernames": username }).await,
        }
    }

    async fn export_peers(&self, limit: Option<usize>) -> io::Result<Vec<PeerRecord>> {
        let peers = self.collection("peers").await?;
        let mut find = peers.find(doc! {}).sort(doc! { "last_update_on": -1, "_id": 1 });
        if let Some(limit) = limit {
            find = find.limit(i64::try_from(limit).unwrap_or(i64::MAX));
        }
        let mut records = Vec::new();
        for d in all(find.await.map_err(io_error)?).await? {
            records.extend(self.record(&d).await?);
        }
        Ok(records)
    }

    async fn clear_peers(&self) -> io::Result<()> {
        let db = self.db().await?;
        for name in ["peers", "usernames"] {
            db.collection::<Document>(name).delete_many(doc! {}).await.map_err(io_error)?;
        }
        Ok(())
    }

    async fn delete(&self) -> io::Result<()> {
        let db = self.db().await?;
        for name in ["session", "update_state", "vaegram", "version", "peers", "usernames"] {
            db.collection::<Document>(name).delete_many(doc! {}).await.map_err(io_error)?;
        }
        Ok(())
    }
}

async fn all(mut cursor: Cursor<Document>) -> io::Result<Vec<Document>> {
    let mut out = Vec::new();
    while cursor.advance().await.map_err(io_error)? {
        out.push(cursor.deserialize_current().map_err(io_error)?);
    }
    Ok(out)
}

fn io_error(e: mongodb::error::Error) -> io::Error {
    let kind = match *e.kind {
        ErrorKind::ServerSelection { .. } => io::ErrorKind::TimedOut,
        ErrorKind::Io(_) => io::ErrorKind::ConnectionAborted,
        ErrorKind::InvalidArgument { .. } => io::ErrorKind::InvalidInput,
        _ => io::ErrorKind::Other,
    };
    io::Error::new(kind, format!("mongodb: {e}"))
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

// Other libraries write numbers as 32- or 64-bit integers, and flags as booleans.
fn int(d: &Document, key: &str) -> Option<i64> {
    match d.get(key)? {
        Bson::Int32(n) => Some(i64::from(*n)),
        Bson::Int64(n) => Some(*n),
        Bson::Double(n) if n.fract() == 0.0 => Some(*n as i64),
        Bson::Boolean(b) => Some(i64::from(*b)),
        _ => None,
    }
}

fn session_document(s: &SessionData, date: i64) -> Document {
    let (address, port) = match s.addr {
        Some(addr) => (Bson::String(addr.ip().to_string()), Bson::Int32(i32::from(addr.port()))),
        None => (Bson::Null, Bson::Null),
    };
    doc! {
        "dc_id": s.dc_id,
        "server_address": address,
        "port": port,
        "api_id": s.api_id,
        "test_mode": s.test_mode,
        "auth_key": Binary { subtype: BinarySubtype::Generic, bytes: s.auth_key.to_vec() },
        "date": date,
        "user_id": s.user_id,
        "is_bot": s.is_bot,
    }
}

fn session(d: &Document) -> Option<SessionData> {
    let auth_key = match d.get("auth_key")? {
        Bson::Binary(b) => <[u8; 256]>::try_from(b.bytes.as_slice()).ok()?,
        _ => return None,
    };
    let mut s = SessionData::new(i32::try_from(int(d, "dc_id")?).ok()?, auth_key);
    let ip = d.get_str("server_address").ok().and_then(|a| a.parse::<IpAddr>().ok());
    let port = int(d, "port").and_then(|p| u16::try_from(p).ok()).filter(|&p| p != 0);
    s.addr = ip.zip(port).map(|(ip, port)| SocketAddr::new(ip, port));
    s.api_id = int(d, "api_id").and_then(|n| i32::try_from(n).ok()).unwrap_or(0);
    s.test_mode = int(d, "test_mode").is_some_and(|v| v != 0);
    s.user_id = int(d, "user_id").unwrap_or(0);
    s.is_bot = int(d, "is_bot").is_some_and(|v| v != 0);
    Some(s)
}

fn update_state(rows: &[Document]) -> Option<UpdateState> {
    let n = |d: &Document, key| int(d, key).and_then(|v| i32::try_from(v).ok()).unwrap_or(0);
    let account = rows.iter().find(|d| int(d, "_id") == Some(0))?;
    let channels = rows
        .iter()
        .filter_map(|d| Some((int(d, "_id").filter(|&id| id <= -CHANNEL_OFFSET)?, n(d, "pts"))))
        .map(|(id, pts)| (get_channel_id(id), pts))
        .collect();
    let update = UpdateState {
        pts: n(account, "pts"),
        qts: n(account, "qts"),
        date: n(account, "date"),
        seq: n(account, "seq"),
        channels,
    };
    (update.date != 0).then_some(update)
}

fn state_rows(state: &StoredState) -> Vec<Document> {
    let Some(u) = state.update.as_ref().filter(|u| u.date != 0) else { return Vec::new() };
    let mut rows = vec![doc! { "_id": 0_i64, "pts": u.pts, "qts": u.qts, "date": u.date, "seq": u.seq }];
    rows.extend(u.channels.iter().map(|(&id, &pts)| doc! { "_id": get_channel_id(id), "pts": pts }));
    rows
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

fn peer_document(p: &PeerRecord) -> Document {
    doc! {
        "_id": p.id,
        "access_hash": p.access_hash,
        "type": peer_type(p.id),
        "phone_number": p.phone_number.as_deref().map_or(Bson::Null, |n| Bson::String(n.to_owned())),
        "last_update_on": p.updated,
        "usernames": p.usernames.clone(),
    }
}

fn peer(d: &Document) -> Option<PeerRecord> {
    let mut p = PeerRecord::new(int(d, "_id")?, int(d, "access_hash").unwrap_or(0));
    p.phone_number = d.get_str("phone_number").ok().filter(|n| !n.is_empty()).map(str::to_owned);
    p.updated = int(d, "last_update_on").unwrap_or(0);
    if let Ok(names) = d.get_array("usernames") {
        p.usernames = names.iter().filter_map(|n| n.as_str().map(str::to_owned)).collect();
    }
    Some(p)
}

impl fmt::Debug for MongoStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MongoStorage").field("database", &self.database).finish_non_exhaustive()
    }
}

impl Storage for MongoStorage {
    fn open(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move { self.db().await.map(drop) })
    }

    fn close(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            self.db.lock().unwrap_or_else(PoisonError::into_inner).take();
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
        Box::pin(self.peer_where(doc! { "_id": id }))
    }

    fn get_peer_by_username<'a>(&'a self, username: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        Box::pin(self.get_peer_by_username(username))
    }

    fn get_peer_by_phone_number<'a>(&'a self, phone_number: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        Box::pin(self.peer_where(doc! { "phone_number": phone_number }))
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
    use std::collections::HashMap;

    use mongodb::bson::spec::BinarySubtype;
    use mongodb::bson::{Binary, Bson, Document, doc};

    use super::{MongoStorage, all, int, peer, peer_document, session, session_document, state_rows, update_state};
    use crate::session::Salt;
    use crate::storage::{PeerRecord, SessionData, Storage, StoredState, UpdateState, conformance};

    #[test]
    fn bson_mapping_round_trips() {
        let mut s = SessionData::new(4, std::array::from_fn(|i| i as u8));
        (s.api_id, s.user_id, s.is_bot, s.test_mode) = (12345, 5_000_000_000, true, true);
        for addr in [None, Some("[2001:db8::1]:443".parse().unwrap())] {
            s.addr = addr;
            let back = session(&session_document(&s, 1)).unwrap();
            let fields = |s: &SessionData| (s.dc_id, s.addr, s.api_id, s.auth_key, s.user_id, s.is_bot, s.test_mode);
            assert_eq!(fields(&back), fields(&s));
        }

        let mut p = PeerRecord::new(-1_000_000_001_234, 6);
        (p.usernames, p.phone_number, p.updated) = (vec!["b".into(), "a".into()], Some("1555".into()), 30);
        let d = peer_document(&p);
        assert_eq!(d.get_str("type").ok(), Some("channel"));
        assert_eq!(peer(&d), Some(p));

        let update = UpdateState { pts: 3, qts: 1, date: 9, seq: 2, channels: HashMap::from([(1234, 8)]) };
        let rows = state_rows(&StoredState::new(42, Salt::default(), Some(update.clone())));
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|r| r.get_i64("_id").ok() == Some(-1_000_000_001_234)));
        assert_eq!(update_state(&rows), Some(update));
        assert!(state_rows(&StoredState::new(42, Salt::default(), None)).is_empty());
    }

    #[test]
    fn reads_documents_another_library_wrote() {
        let key = Binary { subtype: BinarySubtype::Generic, bytes: vec![7; 256] };
        let written = doc! {
            "_id": 0, "dc_id": 2_i32, "server_address": "149.154.167.51", "port": 443_i32, "api_id": 1_i32,
            "test_mode": false, "auth_key": key, "date": 0_i32, "user_id": 42_i32, "is_bot": true,
        };
        let s = session(&written).unwrap();
        assert_eq!((s.dc_id, s.addr, s.user_id, s.is_bot), (2, Some("149.154.167.51:443".parse().unwrap()), 42, true));
        assert!(session(&doc! { "_id": 0, "dc_id": 2, "auth_key": Bson::Null }).is_none(), "no key, no login");

        let p =
            peer(&doc! { "_id": 100_i32, "access_hash": 5_i64, "type": "bot", "phone_number": Bson::Null }).unwrap();
        assert_eq!((p.id, p.access_hash, p.phone_number, p.usernames.len()), (100, 5, None, 0));
        let rows = [doc! { "_id": 0_i32, "pts": 7_i32, "qts": Bson::Null, "date": 1_700_000_000_i32, "seq": 2_i32 }];
        assert_eq!(update_state(&rows).map(|u| (u.pts, u.qts, u.date)), Some((7, 0, 1_700_000_000)));
    }

    #[tokio::test]
    async fn unreachable_server_is_an_io_error() {
        let uri = "mongodb://user:hunter2@127.0.0.1:1/?serverSelectionTimeoutMS=200&connectTimeoutMS=200";
        let storage = MongoStorage::new(uri, "bot");
        let error = storage.load_session().await.err().unwrap();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut, "{error}");
        assert!(!error.to_string().contains("hunter2"), "{error}");
    }

    #[tokio::test]
    async fn debug_hides_the_password() {
        let storage = MongoStorage::new("mongodb://user:hunter2@db.example:27017", "bot").database("sessions");
        let debug = format!("{storage:?}");
        assert!(!debug.contains("hunter2") && debug.contains("sessions"), "{debug}");
        for malformed in
            ["mongodb://user:hunter2@[::1", "mongodb://user:hunter2@host:notaport/", "mongo://user:hunter2@h"]
        {
            let error = MongoStorage::new(malformed, "bot").open().await.unwrap_err();
            assert!(!error.to_string().contains("hunter2"), "{malformed}: {error}");
        }
    }

    #[tokio::test]
    #[ignore = "needs VAEGRAM_MONGO_URL, a MongoDB server it may write to"]
    async fn conformance_on_a_live_server() {
        let Some(uri) = live_uri() else { return };
        let database = format!("vaegram_live_{}", std::process::id());
        conformance::check(&MongoStorage::new(uri.clone(), "bot").database(database.clone())).await;
        mongodb::Client::with_uri_str(uri).await.unwrap().database(&database).drop().await.unwrap();
    }

    // The CI job that runs these has a server; without the variable they would pass having tested nothing.
    fn live_uri() -> Option<String> {
        let uri = std::env::var("VAEGRAM_MONGO_URL").ok();
        assert!(uri.is_some() || std::env::var_os("CI").is_none(), "set VAEGRAM_MONGO_URL");
        uri
    }

    #[tokio::test]
    #[ignore = "needs VAEGRAM_MONGO_URL, a MongoDB server it may write to"]
    async fn conformance_on_a_live_server_survives_interrupted_writes() {
        use std::sync::atomic::Ordering::SeqCst;
        let Some(uri) = live_uri() else { return };
        let database = format!("vaegram_live_faults_{}", std::process::id());
        let storage = MongoStorage::new(uri.clone(), "bot").database(database.clone());
        let fail_after = |writes| storage.writes_left.store(writes, SeqCst);
        let state = |pts, date, channel| {
            let update = UpdateState { pts, date, channels: HashMap::from([(channel, pts)]), ..UpdateState::default() };
            StoredState::new(1, Salt::default(), Some(update))
        };

        // A save writes the owner, the account row, the channel row, then deletes stale rows: stop it after the
        // account row, and after every row.
        for writes in [2, 3] {
            storage.save_state(&state(1, 5, 9)).await.unwrap();
            fail_after(writes);
            let interrupted = storage.save_state(&state(2, 6, 10)).await;
            fail_after(usize::MAX);
            let loaded = storage.load_state().await.unwrap();
            assert!(interrupted.is_err(), "{writes}");
            assert!(loaded.and_then(|s| s.update).is_some_and(|u| u.date != 0), "{writes}: the state went missing");
        }

        // Two peer writes, then "alice" moving to 200 and "alice2" going to 100: stop before each of them.
        let db = mongodb::Client::with_uri_str(&uri).await.unwrap().database(&database);
        for writes in 0..4 {
            storage.delete().await.unwrap();
            let mut alice = PeerRecord::new(100, 1);
            (alice.usernames, alice.updated) = (vec!["alice".into()], 10);
            storage.update_peers(&[alice]).await.unwrap();
            let mut renamed = PeerRecord::new(100, 1);
            (renamed.usernames, renamed.updated) = (vec!["alice2".into()], 20);
            let mut taker = PeerRecord::new(200, 2);
            (taker.usernames, taker.updated) = (vec!["alice".into()], 30);
            fail_after(writes);
            let interrupted = storage.update_peers(&[renamed, taker]).await;
            fail_after(usize::MAX);
            assert!(interrupted.is_err(), "{writes}");
            // Every stored name points at a stored peer, and a lookup gives a peer that lists the name, or nobody
            // once no written peer does.
            let names = all(db.collection::<Document>("usernames").find(doc! {}).await.unwrap()).await.unwrap();
            for d in &names {
                let (name, id) = (d.get_str("_id").unwrap(), int(d, "peer_id").unwrap());
                let peer = db.collection::<Document>("peers").find_one(doc! { "_id": id }).await.unwrap();
                assert!(peer.is_some(), "{writes}: {name} points at no peer");
                let found = storage.get_peer_by_username(name).await.unwrap();
                let listed = found.as_ref().is_none_or(|p| p.usernames.iter().any(|n| n == name));
                assert!(listed, "{writes}: {name} went to {found:?}");
            }
            if writes >= 2 {
                let found = storage.get_peer_by_username("alice").await.unwrap();
                assert_eq!(found.map(|p| p.id), Some(200), "{writes}: both peers are written");
            }
        }
        db.drop().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "needs VAEGRAM_MONGO_URL, a MongoDB server it may write to"]
    async fn conformance_on_a_live_server_skips_a_name_its_peer_dropped() {
        use std::sync::atomic::Ordering::SeqCst;
        let Some(uri) = live_uri() else { return };
        let database = format!("vaegram_live_stale_{}", std::process::id());
        let storage = MongoStorage::new(uri.clone(), "bot").database(database.clone());
        let mut owner = PeerRecord::new(100, 1);
        (owner.usernames, owner.updated) = (vec!["alice".into()], 10);
        let mut older = PeerRecord::new(200, 2);
        (older.usernames, older.updated) = (vec!["alice".into()], 5);
        storage.update_peers(&[owner, older.clone()]).await.unwrap();

        // 100 lets "alice" go, and the save stops after the peer write: the names still say 100.
        let mut renamed = PeerRecord::new(100, 1);
        (renamed.usernames, renamed.updated) = (vec!["alice2".into()], 20);
        storage.writes_left.store(1, SeqCst);
        assert!(storage.update_peers(&[renamed]).await.is_err());
        storage.writes_left.store(usize::MAX, SeqCst);
        let db = mongodb::Client::with_uri_str(&uri).await.unwrap().database(&database);
        let stale = db.collection::<Document>("usernames").find_one(doc! { "_id": "alice" }).await.unwrap();
        assert_eq!(stale.as_ref().and_then(|d| int(d, "peer_id")), Some(100));

        assert_eq!(storage.get_peer_by_username("alice").await.unwrap(), Some(older));
        db.drop().await.unwrap();
    }
}
