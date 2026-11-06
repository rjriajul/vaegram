use std::fmt;
use std::fs::{self, File};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use super::engine::{PeerRecord, Storage, StorageFuture, StorageReporter, StoredState};
use super::memory_storage::Tables;
use super::{Reports, SessionData, create_private, file_format, write_private};

const MIN_COMPACT: usize = 1 << 20;

/// Keeps everything in one file, `<name>.session`, that each save appends to and that is rewritten whole once
/// the appended bytes outgrow what it holds. Every record is also kept in memory.
///
/// The file is created private to the user (0600 on Unix). One client per file: two processes writing the same
/// file would interleave their records. A file vaegram didn't write is refused and left as it is.
pub struct FileStorage {
    path: PathBuf,
    file: Mutex<Open>,
    // Outside `Open`, which `close` resets.
    reports: Reports,
}

#[derive(Default)]
struct Open {
    opened: bool,
    tables: Tables,
    // Whole records of kinds this version skips, kept for a newer vaegram through compaction.
    skipped: Vec<u8>,
    handle: Option<File>,
    // Where the next record goes: the end of the last whole record.
    end: usize,
    // Set after a torn write, so the next write cuts the garbage off first.
    truncate: bool,
    appended: usize,
    // The file's size right after it was last read or rewritten whole.
    live: usize,
    compact_at: usize,
    // After a failed rewrite, how many appended bytes to wait for before trying again.
    retry_at: usize,
}

impl FileStorage {
    /// `name` is what [`Client::builder`](crate::Client::builder) takes, so it can hold a directory:
    /// `"bots/echo"` keeps `bots/echo.session`.
    pub fn new(name: impl AsRef<Path>) -> FileStorage {
        let mut path = name.as_ref().as_os_str().to_owned();
        path.push(".session");
        let file = Mutex::new(Open { compact_at: MIN_COMPACT, ..Open::default() });
        FileStorage { path: PathBuf::from(path), file, reports: Reports::default() }
    }

    /// The session file: the name given to [`new`](Self::new) with `.session` added.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn lock(&self) -> MutexGuard<'_, Open> {
        self.file.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn named(&self, e: io::Error) -> io::Error {
        io::Error::new(e.kind(), format!("{}: {e}", self.path.display()))
    }

    /// Runs `f` on the replayed file, reading it first if it hasn't been.
    fn with<T>(&self, f: impl FnOnce(&mut Open, &Path) -> io::Result<T>) -> io::Result<T> {
        let mut open = self.lock();
        if !open.opened {
            self.read(&mut open).map_err(|e| self.named(e))?;
        }
        f(&mut open, &self.path).map_err(|e| self.named(e))
    }

    fn read(&self, open: &mut Open) -> io::Result<()> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e),
        };
        let replayed = file_format::replay(&bytes)?;
        if replayed.damaged {
            self.reports.report(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{}: dropping a half-written record at byte {}; the next save cuts it off",
                    self.path.display(),
                    replayed.good_len
                ),
            ));
        }
        open.tables = replayed.tables;
        open.skipped = replayed.skipped;
        open.end = replayed.good_len;
        open.live = replayed.good_len;
        open.truncate = replayed.damaged;
        open.appended = 0;
        open.retry_at = 0;
        open.opened = true;
        Ok(())
    }

    /// Appends what `fill` encodes in one write and syncs it; on failure nothing in memory changes.
    fn append(open: &mut Open, path: &Path, fill: impl FnOnce(&mut Vec<u8>)) -> io::Result<()> {
        let mut bytes = Vec::new();
        if open.end == 0 {
            bytes.extend_from_slice(&file_format::header());
        }
        fill(&mut bytes);
        let mut handle = match open.handle.take() {
            Some(handle) => handle,
            None if open.end == 0 => create_private(path)?,
            None => fs::OpenOptions::new().write(true).open(path)?,
        };
        let written = write_at(&mut handle, open.end as u64, open.truncate, &bytes);
        open.handle = Some(handle);
        // Whatever part of a failed write landed is cut off by the next one.
        open.truncate = written.is_err();
        written?;
        open.end += bytes.len();
        open.appended += bytes.len();
        Ok(())
    }

    /// Rewrites the file from memory once the appended records outgrow the live ones.
    fn compact_if_due(&self, open: &mut Open, path: &Path) {
        if open.appended <= open.compact_at.max(open.live).max(open.retry_at) {
            return;
        }
        let bytes = file_format::snapshot(&open.tables, &open.skipped);
        // The rename replaces the file the handle points at; Windows refuses that while it is open.
        open.handle = None;
        match write_private(path, &bytes) {
            Ok(()) => {
                (open.end, open.live, open.appended, open.truncate) = (bytes.len(), bytes.len(), 0, false);
                open.retry_at = 0;
            }
            Err(e) => {
                open.retry_at = open.appended.saturating_mul(2);
                let why = format!("{}: couldn't compact the session file: {e}", path.display());
                self.reports.report(io::Error::new(e.kind(), why));
            }
        }
    }
}

fn write_at(handle: &mut File, at: u64, truncate: bool, bytes: &[u8]) -> io::Result<()> {
    if truncate {
        handle.set_len(at)?;
    }
    handle.seek(SeekFrom::Start(at))?;
    handle.write_all(bytes)?;
    handle.sync_data()
}

impl fmt::Debug for FileStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileStorage").field("path", &self.path).finish_non_exhaustive()
    }
}

impl Storage for FileStorage {
    fn set_reporter(&self, reporter: StorageReporter) {
        self.reports.set(reporter);
    }

    fn open(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move { self.with(|_, _| Ok(())) })
    }

    fn close(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let mut open = self.lock();
            *open = Open { compact_at: open.compact_at, ..Open::default() };
            Ok(())
        })
    }

    fn delete(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let mut open = self.lock();
            open.handle = None;
            super::delete(&self.path).map_err(|e| self.named(e))?;
            *open = Open { opened: true, compact_at: open.compact_at, ..Open::default() };
            Ok(())
        })
    }

    fn load_session(&self) -> StorageFuture<'_, Option<SessionData>> {
        Box::pin(async move { self.with(|open, _| Ok(open.tables.session.clone())) })
    }

    fn save_session<'a>(&'a self, session: &'a SessionData) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.with(|open, path| {
                FileStorage::append(open, path, |b| file_format::session(b, session))?;
                open.tables.session = Some(session.clone());
                self.compact_if_due(open, path);
                Ok(())
            })
        })
    }

    fn load_state(&self) -> StorageFuture<'_, Option<StoredState>> {
        Box::pin(async move { self.with(|open, _| Ok(open.tables.state.clone())) })
    }

    fn save_state<'a>(&'a self, state: &'a StoredState) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.with(|open, path| {
                FileStorage::append(open, path, |b| file_format::state(b, state))?;
                open.tables.state = Some(state.clone());
                self.compact_if_due(open, path);
                Ok(())
            })
        })
    }

    fn update_peers<'a>(&'a self, peers: &'a [PeerRecord]) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            if peers.is_empty() {
                return Ok(());
            }
            self.with(|open, path| {
                FileStorage::append(open, path, |b| peers.iter().for_each(|p| file_format::peer(b, p)))?;
                for p in peers {
                    open.tables.upsert(p.clone());
                }
                self.compact_if_due(open, path);
                Ok(())
            })
        })
    }

    fn get_peer_by_id(&self, id: i64) -> StorageFuture<'_, Option<PeerRecord>> {
        Box::pin(async move { self.with(|open, _| Ok(open.tables.by_id(id))) })
    }

    fn get_peer_by_username<'a>(&'a self, username: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        Box::pin(async move { self.with(|open, _| Ok(open.tables.by_username(username))) })
    }

    fn get_peer_by_phone_number<'a>(&'a self, phone_number: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        Box::pin(async move { self.with(|open, _| Ok(open.tables.by_phone_number(phone_number))) })
    }

    fn export_peers(&self, limit: Option<usize>) -> StorageFuture<'_, Vec<PeerRecord>> {
        Box::pin(async move { self.with(|open, _| Ok(open.tables.export(limit))) })
    }

    fn clear_peers(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            self.with(|open, path| {
                FileStorage::append(open, path, file_format::clear_peers)?;
                open.tables.clear_peers();
                self.compact_if_due(open, path);
                Ok(())
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::FileStorage;
    use crate::session::Salt;
    use crate::storage::{PeerRecord, SessionData, Storage, StoredState, conformance};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vaegram-file-storage-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn peer(id: i64, updated: i64) -> PeerRecord {
        let mut p = PeerRecord::new(id, id * 3);
        (p.usernames, p.updated) = (vec![format!("user{id}")], updated);
        p
    }

    fn session() -> SessionData {
        let mut s = SessionData::new(4, [9; 256]);
        s.user_id = 77;
        s
    }

    #[tokio::test]
    async fn conformance_and_reopen_keeps_everything() {
        let dir = temp_dir("conformance");
        let storage = FileStorage::new(dir.join("bot"));
        conformance::check(&storage).await;
        storage.save_session(&session()).await.unwrap();
        storage.update_peers(&[peer(1, 5), peer(2, 6)]).await.unwrap();
        let state = StoredState::new(77, Salt { value: 1, time_offset: 2 }, None);
        storage.save_state(&state).await.unwrap();
        storage.close().await.unwrap();

        let again = FileStorage::new(dir.join("bot"));
        again.open().await.unwrap();
        let loaded = (again.load_session().await.unwrap().map(|s| s.user_id), again.load_state().await.unwrap());
        let peers = again.export_peers(None).await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(loaded, (Some(77), Some(state)));
        assert_eq!(peers, [peer(2, 6), peer(1, 5)]);
    }

    #[tokio::test]
    async fn torn_tail_is_dropped_and_truncated_on_next_write() {
        let dir = temp_dir("torn");
        let storage = FileStorage::new(dir.join("bot"));
        storage.save_session(&session()).await.unwrap();
        storage.update_peers(&[peer(1, 1)]).await.unwrap();
        let good = std::fs::read(storage.path()).unwrap();
        let mut torn = good.clone();
        torn.extend_from_slice(&[40, 0, 0, 0, 1, 2, 3, 4, 3, 9, 9]);
        std::fs::write(storage.path(), &torn).unwrap();

        let reopened = FileStorage::new(dir.join("bot"));
        let before = reopened.export_peers(None).await.unwrap();
        reopened.update_peers(&[peer(2, 2)]).await.unwrap();
        let after = std::fs::read(reopened.path()).unwrap();
        let replayed = super::file_format::replay(&after).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(before, [peer(1, 1)]);
        assert_eq!(&after[..good.len()], &good[..], "the good records stay as they were");
        assert!(!replayed.damaged, "the torn bytes were cut off before the append");
        assert_eq!(replayed.tables.export(None), [peer(2, 2), peer(1, 1)]);
    }

    #[tokio::test]
    async fn a_torn_tail_is_reported_once() {
        let dir = temp_dir("torn-report");
        let storage = FileStorage::new(dir.join("bot"));
        storage.save_session(&session()).await.unwrap();
        let mut torn = std::fs::read(storage.path()).unwrap();
        torn.extend_from_slice(&[40, 0, 0, 0, 1, 2, 3, 4, 3, 9, 9]);
        std::fs::write(storage.path(), &torn).unwrap();

        let reopened = FileStorage::new(dir.join("bot"));
        let reports = conformance::collect_reports(&reopened);
        reopened.open().await.unwrap();
        reopened.update_peers(&[peer(2, 2)]).await.unwrap();
        reopened.update_peers(&[peer(3, 3)]).await.unwrap();
        let reports = reports.lock().unwrap().clone();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(reports.len(), 1, "{reports:?}");
        assert!(reports[0].contains("dropping a half-written record at byte"), "{reports:?}");
    }

    // A directory where the file was makes the rewrite's rename fail; unix only, as a file in use can't be replaced
    // by one elsewhere.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_failed_compaction_is_reported() {
        let dir = temp_dir("compact-report");
        let storage = FileStorage::new(dir.join("bot"));
        let reports = conformance::collect_reports(&storage);
        // Above the session record's size, so saving it doesn't compact yet.
        storage.lock().compact_at = 1024;
        storage.save_session(&session()).await.unwrap();
        std::fs::remove_file(storage.path()).unwrap();
        std::fs::create_dir(storage.path()).unwrap();
        for updated in 0..200 {
            storage.update_peers(&[peer(1, updated), peer(2, updated)]).await.unwrap();
            if !reports.lock().unwrap().is_empty() {
                break;
            }
        }
        let reports = reports.lock().unwrap().clone();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(reports.len(), 1, "{reports:?}");
        assert!(reports[0].contains("couldn't compact the session file"), "{reports:?}");
    }

    #[tokio::test]
    async fn bad_crc_mid_file_stops_replay() {
        let dir = temp_dir("bad-crc");
        let storage = FileStorage::new(dir.join("bot"));
        storage.save_session(&session()).await.unwrap();
        let after_session = std::fs::read(storage.path()).unwrap().len();
        storage.update_peers(&[peer(1, 1)]).await.unwrap();
        storage.update_peers(&[peer(2, 2)]).await.unwrap();
        let mut bytes = std::fs::read(storage.path()).unwrap();
        bytes[after_session + 12] ^= 0xff;
        std::fs::write(storage.path(), &bytes).unwrap();

        let reopened = FileStorage::new(dir.join("bot"));
        let loaded = (reopened.load_session().await.unwrap().map(|s| s.user_id), reopened.export_peers(None).await);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(loaded.0, Some(77));
        assert_eq!(loaded.1.unwrap(), Vec::<PeerRecord>::new(), "nothing after the damaged record is trusted");
    }

    #[tokio::test]
    async fn compaction_keeps_the_live_set() {
        let dir = temp_dir("compaction");
        let storage = FileStorage::new(dir.join("bot"));
        storage.lock().compact_at = 256;
        storage.save_session(&session()).await.unwrap();
        for updated in 0..200 {
            storage.update_peers(&[peer(1, updated), peer(2, updated)]).await.unwrap();
        }
        storage.clear_peers().await.unwrap();
        storage.update_peers(&[peer(3, 1)]).await.unwrap();
        let size = std::fs::metadata(storage.path()).unwrap().len();
        let reopened = FileStorage::new(dir.join("bot"));
        let loaded = (reopened.load_session().await.unwrap().map(|s| s.user_id), reopened.export_peers(None).await);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(size < 4096, "the file was never rewritten: {size} bytes");
        assert_eq!(loaded.0, Some(77));
        assert_eq!(loaded.1.unwrap(), [peer(3, 1)]);
    }

    #[tokio::test]
    async fn old_text_session_is_refused_untouched() {
        let dir = temp_dir("old-text");
        let storage = FileStorage::new(dir.join("bot"));
        let old = "VAE_BAQAADA5AQABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4fICEiIyQlJicoKSorLC0uLzAx";
        std::fs::write(storage.path(), old).unwrap();
        let opened = storage.open().await;
        let saved = storage.save_session(&session()).await;
        let deleted_peers = storage.clear_peers().await;
        let left = std::fs::read_to_string(storage.path()).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let message = opened.unwrap_err().to_string();
        assert!(message.contains("log in again") && message.contains("bot.session"), "{message}");
        assert!(saved.is_err() && deleted_peers.is_err());
        assert_eq!(left, old);
    }

    #[tokio::test]
    async fn sqlite_file_is_refused_with_a_hint() {
        let dir = temp_dir("sqlite");
        let storage = FileStorage::new(dir.join("bot"));
        let mut sqlite = b"SQLite format 3\0".to_vec();
        sqlite.resize(4096, 0);
        std::fs::write(storage.path(), &sqlite).unwrap();
        let error = storage.load_session().await.err().unwrap();
        let left = std::fs::read(storage.path()).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("SqliteStorage"), "{error}");
        assert_eq!(left, sqlite);
    }

    #[tokio::test]
    async fn delete_removes_the_file_and_a_new_login_starts_over() {
        let dir = temp_dir("delete");
        let storage = FileStorage::new(dir.join("bot"));
        storage.save_session(&session()).await.unwrap();
        storage.delete().await.unwrap();
        let gone = !storage.path().exists();
        storage.update_peers(&[peer(1, 1)]).await.unwrap();
        let replayed = super::file_format::replay(&std::fs::read(storage.path()).unwrap()).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(gone);
        assert!(replayed.tables.session.is_none());
        assert_eq!(replayed.tables.export(None), [peer(1, 1)]);
    }

    #[tokio::test]
    async fn a_moved_username_stays_moved_after_compaction_and_reopen() {
        let dir = temp_dir("moved-username");
        let storage = FileStorage::new(dir.join("bot"));
        let (mut old, mut new) = (PeerRecord::new(100, 1), PeerRecord::new(200, 2));
        (old.usernames, old.updated) = (vec!["shared".into()], 10);
        (new.usernames, new.updated) = (vec!["shared".into()], 20);
        storage.update_peers(&[new, old]).await.unwrap();
        storage.lock().compact_at = 1;
        storage.update_peers(&[peer(3, 1)]).await.unwrap();
        let compacted = storage.lock().appended == 0;
        let reopened = FileStorage::new(dir.join("bot"));
        let owner = reopened.get_peer_by_username("shared").await.unwrap().map(|p| p.id);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(compacted);
        assert_eq!(owner, Some(200));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_failed_compaction_waits_before_trying_again() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("failed-compaction");
        let storage = FileStorage::new(dir.join("bot"));
        storage.save_session(&session()).await.unwrap();
        storage.lock().compact_at = 1;
        // No new file can appear in the directory, so the rewrite fails while appends to the open file work.
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();
        storage.update_peers(&[peer(1, 1)]).await.unwrap();
        let (appended, retry_at) = {
            let open = storage.lock();
            (open.appended, open.retry_at)
        };
        storage.update_peers(&[peer(2, 2)]).await.unwrap();
        let still_waiting = storage.lock().retry_at == retry_at;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(retry_at, appended * 2);
        assert!(still_waiting, "the next write tried again at once");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn file_is_private_on_unix() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("private");
        let storage = FileStorage::new(dir.join("bot"));
        storage.lock().compact_at = 64;
        storage.save_session(&session()).await.unwrap();
        let created = std::fs::metadata(storage.path()).unwrap().permissions().mode() & 0o777;
        for updated in 0..20 {
            storage.update_peers(&[peer(1, updated)]).await.unwrap();
        }
        let compacted = std::fs::metadata(storage.path()).unwrap().permissions().mode() & 0o777;
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!((created, compacted), (0o600, 0o600));
    }
}
