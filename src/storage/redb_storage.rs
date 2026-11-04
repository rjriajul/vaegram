use std::collections::HashMap;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use redb::{Database, MultimapTableDefinition, ReadOnlyTable, ReadableTable, TableDefinition, TableError};

use super::engine::{PeerRecord, Storage, StorageFuture, StoredState};
use super::{SessionData, UpdateState, session_string};
use crate::session::Salt;

const VERSION: u32 = 1;
const MAGIC: &[u8; 9] = b"redb\x1a\x0a\xa9\x0d\x0a";
const FILE_MAGIC: &[u8; 16] = b"vaegram-session\0";

const NEWER: &str = "this redb session needs a newer vaegram";
const FOREIGN: &str = "not a vaegram redb file; it was left as it is";
const FILE_STORAGE: &str = "a FileStorage session file: open it with FileStorage; it was left as it is";
const MALFORMED: &str = "a value in this redb session is malformed";

const META: TableDefinition<&str, &[u8]> = TableDefinition::new("meta");
const PEERS: TableDefinition<i64, &[u8]> = TableDefinition::new("peers");
// (updated, !id): read backwards, the newest peers come first and ties go to the smaller id.
const RECENT: TableDefinition<(i64, i64), ()> = TableDefinition::new("recent");
// Every peer that claims a username or number, as (updated, id): the last one owns it, whatever order the peers
// were written in.
const USERNAMES: MultimapTableDefinition<&str, (i64, i64)> = MultimapTableDefinition::new("usernames");
const PHONES: MultimapTableDefinition<&str, (i64, i64)> = MultimapTableDefinition::new("phones");
const CHANNELS: TableDefinition<i64, i32> = TableDefinition::new("channels");

/// Keeps everything in a [redb](https://docs.rs/redb) database file, indexed by id, username and phone number, so
/// nothing is held in memory and every save is one transaction that lands whole or not at all.
///
/// The file is created private to the user (0600 on Unix). One process per file: redb refuses a second opener. A
/// file that isn't a vaegram redb database is refused and left as it is, with one exception: a redb database that
/// another program left after an unclean shutdown is first repaired by redb, and only then refused (its tables
/// and their contents stay as they were). Logging out removes the file.
pub struct RedbStorage {
    source: Source,
    db: Mutex<Option<Database>>,
}

#[derive(Debug)]
enum Source {
    File(PathBuf),
    #[cfg(test)]
    Memory(tests::Disk),
}

impl RedbStorage {
    /// Nothing is read or created until the storage is opened.
    pub fn new(path: impl Into<PathBuf>) -> RedbStorage {
        RedbStorage { source: Source::File(path.into()), db: Mutex::new(None) }
    }

    /// Runs `f` on the open database, opening it first. A failed write closes it, so the next call reopens the
    /// file from its last commit instead of trusting what the failure left in memory.
    fn with<T>(&self, write: bool, f: impl FnOnce(&Database) -> io::Result<T>) -> io::Result<T> {
        let mut slot = self.db.lock().unwrap_or_else(PoisonError::into_inner);
        let db = match slot.take() {
            Some(db) => db,
            None => self.open_database().map_err(|e| self.named(e))?,
        };
        let result = f(&db).map_err(|e| self.named(e));
        if !write || result.is_ok() {
            *slot = Some(db);
        }
        result
    }

    fn named(&self, e: io::Error) -> io::Error {
        match &self.source {
            Source::File(path) => io::Error::new(e.kind(), format!("{}: {e}", path.display())),
            #[cfg(test)]
            Source::Memory(_) => e,
        }
    }

    fn open_database(&self) -> io::Result<Database> {
        let db = match &self.source {
            Source::File(path) => {
                prepare(path)?;
                Database::create(path).io()?
            }
            #[cfg(test)]
            Source::Memory(disk) => Database::builder().create_with_backend(disk.clone()).io()?,
        };
        init(&db)?;
        Ok(db)
    }

    fn remove(&self) -> io::Result<()> {
        self.db.lock().unwrap_or_else(PoisonError::into_inner).take();
        match &self.source {
            Source::File(path) => super::delete(path).map_err(|e| self.named(e)),
            #[cfg(test)]
            Source::Memory(disk) => {
                disk.erase();
                Ok(())
            }
        }
    }
}

/// Refuses a file redb didn't write, before redb touches it, and makes ours private.
fn prepare(path: &std::path::Path) -> io::Result<()> {
    let mut head = Vec::with_capacity(FILE_MAGIC.len());
    match File::open(path) {
        Ok(file) => file.take(FILE_MAGIC.len() as u64).read_to_end(&mut head)?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
            return match options.open(path) {
                Err(e) if e.kind() != io::ErrorKind::AlreadyExists => Err(e),
                _ => prepare(path),
            };
        }
        Err(e) => return Err(e),
    };
    let n = head.len().min(MAGIC.len());
    if head[..n] != MAGIC[..n] {
        let why = if head.starts_with(FILE_MAGIC) { FILE_STORAGE } else { FOREIGN };
        return Err(io::Error::new(io::ErrorKind::InvalidData, why));
    }
    // `mode` only applies when the file is created; one left from before could be world-readable.
    #[cfg(unix)]
    fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
    Ok(())
}

/// Checks the version and creates the tables of a new database. A newer or foreign database is refused before
/// anything is written to it.
fn init(db: &Database) -> io::Result<()> {
    let read = db.begin_read().io()?;
    let version = match read.open_table(META) {
        Ok(meta) => meta.get("version").io()?.map(|v| decode_version(v.value())).transpose()?,
        Err(TableError::TableDoesNotExist(_)) => None,
        Err(e) => return Err(error(e)),
    };
    match version {
        Some(v) if v > VERSION => return Err(io::Error::new(io::ErrorKind::InvalidData, NEWER)),
        Some(_) => return Ok(()),
        None if read.list_tables().io()?.next().is_some() || read.list_multimap_tables().io()?.next().is_some() => {
            return Err(io::Error::new(io::ErrorKind::InvalidData, FOREIGN));
        }
        None => {}
    }
    drop(read);
    let txn = db.begin_write().io()?;
    txn.open_table(META).io()?.insert("version", &VERSION.to_le_bytes()[..]).io()?;
    txn.open_table(PEERS).io()?;
    txn.open_table(RECENT).io()?;
    txn.open_multimap_table(USERNAMES).io()?;
    txn.open_multimap_table(PHONES).io()?;
    txn.open_table(CHANNELS).io()?;
    txn.commit().io()
}

fn decode_version(b: &[u8]) -> io::Result<u32> {
    <[u8; 4]>::try_from(b).map(u32::from_le_bytes).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, MALFORMED))
}

trait Redb<T> {
    fn io(self) -> io::Result<T>;
}

impl<T, E: Into<redb::Error>> Redb<T> for Result<T, E> {
    fn io(self) -> io::Result<T> {
        self.map_err(error)
    }
}

fn error(e: impl Into<redb::Error>) -> io::Error {
    let e = e.into();
    let kind = match &e {
        redb::Error::Io(e) => e.kind(),
        redb::Error::DatabaseAlreadyOpen => io::ErrorKind::ResourceBusy,
        redb::Error::Corrupted(_)
        | redb::Error::UpgradeRequired(_)
        | redb::Error::TableTypeMismatch { .. }
        | redb::Error::TableIsMultimap(_)
        | redb::Error::TableIsNotMultimap(_)
        | redb::Error::TypeDefinitionChanged { .. } => io::ErrorKind::InvalidData,
        _ => io::ErrorKind::Other,
    };
    io::Error::new(kind, format!("redb: {e}"))
}

fn malformed() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, MALFORMED)
}

fn short(s: &str) -> bool {
    s.len() <= u8::MAX as usize
}

/// The usernames a record keeps: Telegram's are far shorter than a length byte allows; longer ones aren't real.
fn usernames(p: &PeerRecord) -> impl Iterator<Item = &str> {
    p.usernames.iter().map(String::as_str).filter(|u| short(u)).take(u8::MAX as usize)
}

fn phone(p: &PeerRecord) -> Option<&str> {
    p.phone_number.as_deref().filter(|n| short(n) && !n.is_empty())
}

fn push_str(b: &mut Vec<u8>, s: &str) {
    b.push(s.len() as u8);
    b.extend_from_slice(s.as_bytes());
}

fn encode_peer(p: &PeerRecord) -> Vec<u8> {
    let mut b = Vec::with_capacity(16 + 2 + 33 * p.usernames.len() + p.phone_number.as_ref().map_or(0, String::len));
    b.extend_from_slice(&p.access_hash.to_le_bytes());
    b.extend_from_slice(&p.updated.to_le_bytes());
    push_str(&mut b, phone(p).unwrap_or(""));
    let names: Vec<&str> = usernames(p).collect();
    b.push(names.len() as u8);
    for name in names {
        push_str(&mut b, name);
    }
    b
}

fn take<const N: usize>(b: &mut &[u8]) -> Option<[u8; N]> {
    let (head, rest) = b.split_first_chunk::<N>()?;
    *b = rest;
    Some(*head)
}

fn take_str(b: &mut &[u8]) -> Option<String> {
    let [len] = take::<1>(b)?;
    let (s, rest) = b.split_at_checked(len as usize)?;
    *b = rest;
    String::from_utf8(s.to_vec()).ok()
}

fn decode_peer(id: i64, mut b: &[u8]) -> Option<PeerRecord> {
    let mut p = PeerRecord::new(id, i64::from_le_bytes(take(&mut b)?));
    p.updated = i64::from_le_bytes(take(&mut b)?);
    p.phone_number = Some(take_str(&mut b)?).filter(|n| !n.is_empty());
    let [count] = take::<1>(&mut b)?;
    p.usernames = (0..count).map(|_| take_str(&mut b)).collect::<Option<_>>()?;
    b.is_empty().then_some(p)
}

fn peer(peers: &impl ReadableTable<i64, &'static [u8]>, id: i64) -> io::Result<Option<PeerRecord>> {
    match peers.get(id).io()? {
        Some(v) => decode_peer(id, v.value()).map(Some).ok_or_else(malformed),
        None => Ok(None),
    }
}

/// Owner of `key`: the newest claim whose peer still names it. Claims a malformed record left behind are skipped.
fn claimed(
    db: &Database,
    index: MultimapTableDefinition<'static, &'static str, (i64, i64)>,
    key: &str,
    names: impl Fn(&PeerRecord) -> bool,
) -> io::Result<Option<PeerRecord>> {
    let read = db.begin_read().io()?;
    let (claims, peers) = (read.open_multimap_table(index).io()?, read.open_table(PEERS).io()?);
    for claim in claims.get(key).io()?.rev() {
        let (updated, id) = claim.io()?.value();
        if let Some(p) = peer(&peers, id)?.filter(|p| p.updated == updated && names(p)) {
            return Ok(Some(p));
        }
    }
    Ok(None)
}

fn load_session(db: &Database) -> io::Result<Option<SessionData>> {
    let read = db.begin_read().io()?;
    let meta = read.open_table(META).io()?;
    let Some(fields) = meta.get("session").io()? else { return Ok(None) };
    match session_string::decode_fields(fields.value()) {
        Ok(s) => Ok(Some(s)),
        Err(why) if why == session_string::NEWER_VAEGRAM => Err(io::Error::new(io::ErrorKind::InvalidData, NEWER)),
        Err(_) => Err(malformed()),
    }
}

fn save_session(db: &Database, s: &SessionData) -> io::Result<()> {
    let txn = db.begin_write().io()?;
    txn.open_table(META).io()?.insert("session", session_string::encode_fields(s).as_slice()).io()?;
    txn.commit().io()
}

fn load_state(db: &Database) -> io::Result<Option<StoredState>> {
    let read = db.begin_read().io()?;
    let meta = read.open_table(META).io()?;
    let Some(row) = meta.get("state").io()? else { return Ok(None) };
    let mut b = row.value();
    let mut i64s = [0; 3];
    for n in &mut i64s {
        *n = i64::from_le_bytes(take(&mut b).ok_or_else(malformed)?);
    }
    let mut i32s = [0; 4];
    for n in &mut i32s {
        *n = i32::from_le_bytes(take(&mut b).ok_or_else(malformed)?);
    }
    if !b.is_empty() {
        return Err(malformed());
    }
    let ([owner, value, time_offset], [pts, qts, date, seq]) = (i64s, i32s);
    let mut update = None;
    if date != 0 {
        let channels: ReadOnlyTable<i64, i32> = read.open_table(CHANNELS).io()?;
        let channels = channels
            .iter()
            .io()?
            .map(|row| row.io().map(|(id, pts)| (id.value(), pts.value())))
            .collect::<io::Result<_>>()?;
        update = Some(UpdateState { pts, qts, date, seq, channels });
    }
    Ok(Some(StoredState { owner, salt: Salt { value, time_offset }, update }))
}

fn save_state(db: &Database, state: &StoredState) -> io::Result<()> {
    let none = UpdateState::default();
    let update = state.update.as_ref().unwrap_or(&none);
    let mut row = Vec::with_capacity(40);
    for n in [state.owner, state.salt.value, state.salt.time_offset] {
        row.extend_from_slice(&n.to_le_bytes());
    }
    for n in [update.pts, update.qts, update.date, update.seq] {
        row.extend_from_slice(&n.to_le_bytes());
    }
    let txn = db.begin_write().io()?;
    txn.open_table(META).io()?.insert("state", row.as_slice()).io()?;
    {
        let mut channels = txn.open_table(CHANNELS).io()?;
        channels.retain(|_, _| false).io()?;
        for (&id, &pts) in &update.channels {
            channels.insert(id, pts).io()?;
        }
    }
    txn.commit().io()
}

fn update_peers(db: &Database, batch: &[PeerRecord]) -> io::Result<()> {
    // The last record of each peer in the batch is the one kept.
    let batch: HashMap<i64, &PeerRecord> = batch.iter().map(|p| (p.id, p)).collect();
    let txn = db.begin_write().io()?;
    {
        let mut peers = txn.open_table(PEERS).io()?;
        let mut recent = txn.open_table(RECENT).io()?;
        let mut names = txn.open_multimap_table(USERNAMES).io()?;
        let mut phones = txn.open_multimap_table(PHONES).io()?;
        for p in batch.values() {
            // A malformed old record is overwritten; the claims it left are skipped on lookup.
            let old = peers.get(p.id).io()?.and_then(|v| decode_peer(p.id, v.value()));
            if let Some(old) = old {
                let claim = (old.updated, old.id);
                for name in usernames(&old) {
                    names.remove(name, claim).io()?;
                }
                if let Some(number) = phone(&old) {
                    phones.remove(number, claim).io()?;
                }
                recent.remove((old.updated, !old.id)).io()?;
            }
            peers.insert(p.id, encode_peer(p).as_slice()).io()?;
            recent.insert((p.updated, !p.id), ()).io()?;
            let claim = (p.updated, p.id);
            for name in usernames(p) {
                names.insert(name, claim).io()?;
            }
            if let Some(number) = phone(p) {
                phones.insert(number, claim).io()?;
            }
        }
    }
    txn.commit().io()
}

fn export_peers(db: &Database, limit: Option<usize>) -> io::Result<Vec<PeerRecord>> {
    let read = db.begin_read().io()?;
    let (recent, peers) = (read.open_table(RECENT).io()?, read.open_table(PEERS).io()?);
    let mut out = Vec::new();
    for row in recent.iter().io()?.rev() {
        if out.len() >= limit.unwrap_or(usize::MAX) {
            break;
        }
        let (updated, id) = row.io()?.0.value();
        // An entry a malformed record left behind points at a peer saved since with another time.
        if let Some(p) = peer(&peers, !id)?.filter(|p| p.updated == updated) {
            out.push(p);
        }
    }
    Ok(out)
}

fn clear_peers(db: &Database) -> io::Result<()> {
    let txn = db.begin_write().io()?;
    txn.open_table(PEERS).io()?.retain(|_, _| false).io()?;
    txn.open_table(RECENT).io()?.retain(|_, _| false).io()?;
    for index in [USERNAMES, PHONES] {
        txn.delete_multimap_table(index).io()?;
        txn.open_multimap_table(index).io()?;
    }
    txn.commit().io()
}

impl fmt::Debug for RedbStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("RedbStorage");
        match &self.source {
            Source::File(path) => d.field("path", path),
            #[cfg(test)]
            Source::Memory(_) => d.field("path", &"<memory>"),
        };
        d.finish_non_exhaustive()
    }
}

impl Storage for RedbStorage {
    fn open(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move { self.with(false, |_| Ok(())) })
    }

    fn close(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            self.db.lock().unwrap_or_else(PoisonError::into_inner).take();
            Ok(())
        })
    }

    fn delete(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move { self.remove() })
    }

    fn load_session(&self) -> StorageFuture<'_, Option<SessionData>> {
        Box::pin(async move { self.with(false, load_session) })
    }

    fn save_session<'a>(&'a self, session: &'a SessionData) -> StorageFuture<'a, ()> {
        Box::pin(async move { self.with(true, |db| save_session(db, session)) })
    }

    fn load_state(&self) -> StorageFuture<'_, Option<StoredState>> {
        Box::pin(async move { self.with(false, load_state) })
    }

    fn save_state<'a>(&'a self, state: &'a StoredState) -> StorageFuture<'a, ()> {
        Box::pin(async move { self.with(true, |db| save_state(db, state)) })
    }

    fn update_peers<'a>(&'a self, peers: &'a [PeerRecord]) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            if peers.is_empty() {
                return Ok(());
            }
            self.with(true, |db| update_peers(db, peers))
        })
    }

    fn get_peer_by_id(&self, id: i64) -> StorageFuture<'_, Option<PeerRecord>> {
        Box::pin(async move {
            self.with(false, |db| {
                let read = db.begin_read().io()?;
                peer(&read.open_table(PEERS).io()?, id)
            })
        })
    }

    fn get_peer_by_username<'a>(&'a self, username: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        Box::pin(async move {
            self.with(false, |db| claimed(db, USERNAMES, username, |p| usernames(p).any(|u| u == username)))
        })
    }

    fn get_peer_by_phone_number<'a>(&'a self, phone_number: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        Box::pin(
            async move { self.with(false, |db| claimed(db, PHONES, phone_number, |p| phone(p) == Some(phone_number))) },
        )
    }

    fn export_peers(&self, limit: Option<usize>) -> StorageFuture<'_, Vec<PeerRecord>> {
        Box::pin(async move { self.with(false, |db| export_peers(db, limit)) })
    }

    fn clear_peers(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move { self.with(true, clear_peers) })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex, MutexGuard};

    use redb::{Database, StorageBackend, TableDefinition, TableHandle};

    use super::{META, PEERS, RedbStorage, Source};
    use crate::session::Salt;
    use crate::storage::{PeerRecord, SessionData, Storage, StoredState, UpdateState, conformance};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vaegram-redb-storage-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn peer(id: i64, names: &[&str], updated: i64) -> PeerRecord {
        let mut p = PeerRecord::new(id, id * 3);
        (p.usernames, p.updated) = (names.iter().map(|n| n.to_string()).collect(), updated);
        p
    }

    /// A disk in memory that fails every write once its budget runs out, as a full or failing disk would.
    #[derive(Clone, Debug, Default)]
    pub(super) struct Disk(Arc<Mutex<DiskState>>);

    #[derive(Debug, Default)]
    struct DiskState {
        bytes: Vec<u8>,
        budget: Option<usize>,
    }

    impl Disk {
        pub(super) fn erase(&self) {
            self.0.lock().unwrap().bytes.clear();
        }

        fn spend(&self) -> io::Result<MutexGuard<'_, DiskState>> {
            let mut state = self.0.lock().unwrap();
            match &mut state.budget {
                Some(0) => return Err(io::Error::other("the disk failed")),
                Some(n) => *n -= 1,
                None => {}
            }
            Ok(state)
        }
    }

    impl StorageBackend for Disk {
        fn len(&self) -> io::Result<u64> {
            Ok(self.0.lock().unwrap().bytes.len() as u64)
        }

        fn read(&self, offset: u64, len: usize) -> io::Result<Vec<u8>> {
            let state = self.0.lock().unwrap();
            let start = usize::try_from(offset).map_err(io::Error::other)?;
            let range = state.bytes.get(start..start.saturating_add(len));
            range.map(<[u8]>::to_vec).ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "past the end"))
        }

        fn set_len(&self, len: u64) -> io::Result<()> {
            self.spend()?.bytes.resize(len as usize, 0);
            Ok(())
        }

        fn sync_data(&self, _: bool) -> io::Result<()> {
            self.spend().map(drop)
        }

        fn write(&self, offset: u64, data: &[u8]) -> io::Result<()> {
            let mut state = self.spend()?;
            let start = offset as usize;
            if state.bytes.len() < start + data.len() {
                state.bytes.resize(start + data.len(), 0);
            }
            state.bytes[start..start + data.len()].copy_from_slice(data);
            Ok(())
        }
    }

    fn in_memory(disk: &Disk) -> RedbStorage {
        RedbStorage { source: Source::Memory(disk.clone()), db: Mutex::new(None) }
    }

    #[tokio::test]
    async fn conformance_and_reopen_keeps_everything() {
        let dir = temp_dir("conformance");
        let storage = RedbStorage::new(dir.join("bot.redb"));
        conformance::check(&storage).await;
        let gone = !dir.join("bot.redb").exists();

        let mut session = SessionData::new(4, [9; 256]);
        session.user_id = 77;
        storage.save_session(&session).await.unwrap();
        storage.update_peers(&[peer(1, &["one"], 5), peer(2, &["two"], 6)]).await.unwrap();
        let update = UpdateState { pts: 3, date: 9, channels: HashMap::from([(5, 6)]), ..UpdateState::default() };
        let state = StoredState::new(77, Salt { value: 1, time_offset: 2 }, Some(update));
        storage.save_state(&state).await.unwrap();
        storage.close().await.unwrap();

        let again = RedbStorage::new(dir.join("bot.redb"));
        again.open().await.unwrap();
        let loaded = (again.load_session().await.unwrap().map(|s| s.user_id), again.load_state().await.unwrap());
        let peers = again.export_peers(None).await.unwrap();
        let by_name = again.get_peer_by_username("one").await.unwrap();
        again.close().await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(gone, "delete removes the file");
        assert_eq!(loaded, (Some(77), Some(state)));
        assert_eq!(peers, [peer(2, &["two"], 6), peer(1, &["one"], 5)]);
        assert_eq!(by_name, Some(peer(1, &["one"], 5)));
    }

    #[tokio::test]
    async fn conformance_in_memory() {
        conformance::check(&in_memory(&Disk::default())).await;
    }

    #[tokio::test]
    async fn the_newest_claim_wins_whatever_the_order() {
        let storage = in_memory(&Disk::default());
        storage.update_peers(&[peer(2, &["name"], 40)]).await.unwrap();
        storage.update_peers(&[peer(1, &["name"], 30)]).await.unwrap();
        assert_eq!(storage.get_peer_by_username("name").await.unwrap().map(|p| p.id), Some(2));
        storage.update_peers(&[peer(2, &["other"], 50)]).await.unwrap();
        assert_eq!(storage.get_peer_by_username("name").await.unwrap().map(|p| p.id), Some(1), "the next claim");
        storage.update_peers(&[peer(3, &["name"], 30), peer(4, &["name"], 30)]).await.unwrap();
        assert_eq!(storage.get_peer_by_username("name").await.unwrap().map(|p| p.id), Some(4), "ties: larger id");
    }

    #[tokio::test]
    async fn a_failed_write_leaves_the_last_commit() {
        let disk = Disk::default();
        let storage = in_memory(&disk);
        let old_state = StoredState::new(7, Salt::default(), Some(UpdateState { date: 1, ..UpdateState::default() }));
        storage.save_state(&old_state).await.unwrap();
        storage.update_peers(&[peer(1, &["alice"], 1), peer(2, &["bob"], 1)]).await.unwrap();
        storage.close().await.unwrap();
        let baseline = disk.0.lock().unwrap().bytes.clone();

        let update = UpdateState { pts: 5, date: 2, channels: HashMap::from([(9, 4)]), ..UpdateState::default() };
        let new_state = StoredState::new(7, Salt { value: 3, time_offset: 0 }, Some(update));
        let new_peers = [peer(1, &["carol"], 2), peer(3, &["alice"], 2)];
        let mut failures = 0;
        for budget in 0.. {
            *disk.0.lock().unwrap() = DiskState { bytes: baseline.clone(), budget: Some(budget) };
            let storage = in_memory(&disk);
            let mut wrote = storage.save_state(&new_state).await;
            if wrote.is_ok() {
                wrote = storage.update_peers(&new_peers).await;
            }
            // Whatever the failure left on the disk, a fresh open sees one commit or the other, never a mix.
            disk.0.lock().unwrap().budget = None;
            storage.close().await.unwrap();
            let state = storage.load_state().await.unwrap();
            let peers = storage.export_peers(None).await.unwrap();
            let alice = storage.get_peer_by_username("alice").await.unwrap().map(|p| p.id);
            let carol = storage.get_peer_by_username("carol").await.unwrap().map(|p| p.id);
            if wrote.is_ok() {
                assert_eq!(state, Some(new_state.clone()));
                assert_eq!((alice, carol), (Some(3), Some(1)));
                assert_eq!(peers.len(), 3);
                break;
            }
            failures += 1;
            assert!(state == Some(old_state.clone()) || state == Some(new_state.clone()), "budget {budget}: {state:?}");
            match (alice, carol) {
                (Some(1), None) => assert_eq!(peers, [peer(1, &["alice"], 1), peer(2, &["bob"], 1)]),
                (Some(3), Some(1)) => assert_eq!(peers.len(), 3),
                other => panic!("budget {budget}: half a batch landed: {other:?}"),
            }
        }
        assert!(failures > 3, "the disk failed at only {failures} points");
    }

    #[tokio::test]
    async fn refuses_files_it_did_not_write_untouched() {
        let dir = temp_dir("foreign");
        let mut cases = vec![
            (b"vaegram-session\0\x01\0\0\0".to_vec(), "FileStorage"),
            (b"SQLite format 3\0".to_vec(), "not a vaegram redb file"),
        ];
        // A redb database another program keeps its own tables in.
        let other = dir.join("other.redb");
        let db = Database::create(&other).unwrap();
        let txn = db.begin_write().unwrap();
        txn.open_table(TableDefinition::<u64, u64>::new("theirs")).unwrap().insert(1, 2).unwrap();
        txn.commit().unwrap();
        drop(db);
        cases.push((std::fs::read(&other).unwrap(), "not a vaegram redb file"));

        for (i, (bytes, why)) in cases.into_iter().enumerate() {
            let path = dir.join(format!("{i}.session"));
            std::fs::write(&path, &bytes).unwrap();
            let error = RedbStorage::new(&path).open().await.unwrap_err();
            let after = std::fs::read(&path).unwrap();
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            assert!(error.to_string().contains(why), "{error}");
            if i < 2 {
                assert_eq!(after, bytes, "the file was left as it is");
            } else {
                let db = Database::create(&path).unwrap();
                let read = db.begin_read().unwrap();
                let tables: Vec<String> = read.list_tables().unwrap().map(|t| t.name().to_owned()).collect();
                assert_eq!(tables, ["theirs"], "no table was added");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_newer_version_is_refused_without_writing() {
        let disk = Disk::default();
        let storage = in_memory(&disk);
        storage.save_session(&SessionData::new(2, [1; 256])).await.unwrap();
        storage.close().await.unwrap();
        let db = Database::builder().create_with_backend(disk.clone()).unwrap();
        let txn = db.begin_write().unwrap();
        txn.open_table(META).unwrap().insert("version", &2u32.to_le_bytes()[..]).unwrap();
        txn.commit().unwrap();
        drop(db);

        let error = storage.save_session(&SessionData::new(3, [2; 256])).await.unwrap_err();
        assert!(error.to_string().contains("newer vaegram"), "{error}");
        let db = Database::builder().create_with_backend(disk.clone()).unwrap();
        let read = db.begin_read().unwrap();
        let meta = read.open_table(META).unwrap();
        let session = meta.get("session").unwrap().unwrap().value().to_vec();
        assert_eq!(super::session_string::decode_fields(&session).map(|s| s.dc_id).ok(), Some(2), "kept as it was");
    }

    #[tokio::test]
    async fn a_malformed_peer_is_an_error_not_a_panic() {
        let disk = Disk::default();
        let storage = in_memory(&disk);
        storage.update_peers(&[peer(1, &["one"], 1)]).await.unwrap();
        storage.close().await.unwrap();
        let db = Database::builder().create_with_backend(disk.clone()).unwrap();
        let txn = db.begin_write().unwrap();
        txn.open_table(PEERS).unwrap().insert(1, &[1, 2, 3][..]).unwrap();
        txn.open_table(META).unwrap().insert("state", &[0; 7][..]).unwrap();
        txn.commit().unwrap();
        drop(db);

        assert_eq!(storage.get_peer_by_id(1).await.unwrap_err().kind(), io::ErrorKind::InvalidData);
        assert_eq!(storage.load_state().await.unwrap_err().kind(), io::ErrorKind::InvalidData);
        storage.update_peers(&[peer(1, &["fresh"], 2)]).await.unwrap();
        assert_eq!(storage.get_peer_by_username("one").await.unwrap(), None, "the old claim is skipped");
        assert_eq!(storage.export_peers(None).await.unwrap(), [peer(1, &["fresh"], 2)]);
    }

    #[tokio::test]
    async fn debug_shows_only_the_path() {
        let storage = RedbStorage::new("bots/echo.redb");
        assert_eq!(format!("{storage:?}"), r#"RedbStorage { path: "bots/echo.redb", .. }"#);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn file_is_private_on_unix() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("private");
        let (new, old) = (dir.join("new.redb"), dir.join("old.redb"));
        RedbStorage::new(&new).open().await.unwrap();
        let storage = RedbStorage::new(&old);
        storage.open().await.unwrap();
        storage.close().await.unwrap();
        std::fs::set_permissions(&old, std::fs::Permissions::from_mode(0o644)).unwrap();
        storage.open().await.unwrap();
        let modes = [&new, &old].map(|p| std::fs::metadata(p).unwrap().permissions().mode() & 0o777);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(modes, [0o600, 0o600]);
    }

    #[test]
    fn peer_bytes_round_trip_and_reject_damage() {
        let mut p = peer(5, &["a", "bb"], 9);
        p.phone_number = Some("1555".into());
        let bytes = super::encode_peer(&p);
        assert_eq!(super::decode_peer(5, &bytes), Some(p));
        for cut in 0..bytes.len() {
            assert_eq!(super::decode_peer(5, &bytes[..cut]), None, "cut at {cut}");
        }
        let mut long = bytes.clone();
        long.push(0);
        assert_eq!(super::decode_peer(5, &long), None);
    }
}
