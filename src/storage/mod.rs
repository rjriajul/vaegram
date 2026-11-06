#[cfg(test)]
pub(crate) mod conformance;
mod copy;
mod engine;
mod file_format;
mod file_storage;
mod hybrid_storage;
mod memory_storage;
#[cfg(feature = "mongodb")]
mod mongo_storage;
mod peer_cache;
pub mod recording;
#[cfg(feature = "redb")]
mod redb_storage;
#[cfg(feature = "redis")]
mod redis_storage;
#[cfg(feature = "s3")]
mod s3_storage;
pub mod session_string;
#[cfg(feature = "sqlite")]
mod sqlite_storage;
mod store;
#[cfg(feature = "vault")]
mod vault_storage;

pub use copy::copy;
pub(crate) use engine::Reports;
pub use engine::{PeerRecord, Storage, StorageFuture, StorageReporter, StoredState};
pub use file_storage::FileStorage;
pub use hybrid_storage::HybridStorage;
pub use memory_storage::MemoryStorage;
#[cfg(feature = "mongodb")]
pub use mongo_storage::MongoStorage;
pub(crate) use peer_cache::{PeerCache, USERNAME_TTL, input_peer};
#[cfg(feature = "redb")]
pub use redb_storage::RedbStorage;
#[cfg(feature = "redis")]
pub use redis_storage::RedisStorage;
#[cfg(feature = "s3")]
pub use s3_storage::S3Storage;
#[cfg(feature = "sqlite")]
pub use sqlite_storage::SqliteStorage;
#[cfg(test)]
pub(crate) use store::Recording;
pub(crate) use store::Store;
#[cfg(feature = "vault")]
pub use vault_storage::VaultStorage;

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::net::SocketAddr;
use std::path::Path;

use crate::crypto::random;

/// Where updates resume after a restart: the account's pts, qts, date and seq, and each channel's pts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpdateState {
    pub pts: i32,
    pub qts: i32,
    pub date: i32,
    pub seq: i32,
    pub channels: HashMap<i64, i32>,
}

impl UpdateState {
    /// The account's state with no channel state yet.
    pub fn new(pts: i32, qts: i32, date: i32, seq: i32) -> Self {
        UpdateState { pts, qts, date, seq, channels: HashMap::new() }
    }

    /// Sets each channel's pts, keyed by channel id, replacing any set before.
    #[must_use]
    pub fn channels(mut self, channels: impl IntoIterator<Item = (i64, i32)>) -> Self {
        self.channels = channels.into_iter().collect();
        self
    }
}

/// A login as a session string carries it. Build one with [`SessionData::new`] and set the other fields.
#[non_exhaustive]
#[derive(Clone)]
pub struct SessionData {
    pub dc_id: i32,
    pub test_mode: bool,
    pub addr: Option<SocketAddr>,
    pub api_id: i32,
    pub auth_key: [u8; 256],
    pub user_id: i64,
    pub is_bot: bool,
}

impl SessionData {
    /// A production-DC login with no address, api id or user yet.
    pub fn new(dc_id: i32, auth_key: [u8; 256]) -> SessionData {
        SessionData { dc_id, test_mode: false, addr: None, api_id: 0, auth_key, user_id: 0, is_bot: false }
    }
}

pub(crate) fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    // A fresh name opened with create_new never reuses a file or follows a link someone left at a guessable name,
    // and the 0600 mode is set when it is created, before the key is in it.
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".{:016x}.tmp", u64::from_le_bytes(random())));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&tmp)?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    // Without the sync, a power loss after the rename can leave an empty file on ext4 and xfs.
    let result = written.and_then(|()| fs::rename(&tmp, path));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Creates `path` as a new file only the owner can read, replacing whatever regular file or symlink is there.
///
/// The old entry is removed and the new file made with `create_new`, so it never reuses an inode someone else may
/// hold open and never follows a link to somewhere else. A directory or other non-regular file is refused. If
/// something reappears at `path` between the two steps, the open fails with `AlreadyExists`.
pub(crate) fn create_private(path: &Path) -> io::Result<fs::File> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() || meta.is_symlink() => delete(path)?,
        Ok(_) => {
            let why = format!("{} is not a regular file", path.display());
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, why));
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

pub(crate) fn delete(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::{create_private, write_private};
    use std::io::Write;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vaegram-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn leaves_a_file_at_the_old_temp_name_alone() {
        let dir = std::env::temp_dir().join(format!("vaegram-write-private-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (path, planted) = (dir.join("bot.session"), dir.join("bot.session.tmp"));
        std::fs::write(&planted, b"planted").unwrap();
        let written = write_private(&path, b"key");
        let (saved, left) = (std::fs::read(&path), std::fs::read(&planted));
        let leftovers = std::fs::read_dir(&dir).unwrap().count();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(written.is_ok(), "{written:?}");
        assert_eq!((saved.unwrap(), left.ok(), leftovers), (b"key".to_vec(), Some(b"planted".to_vec()), 2));
    }

    #[test]
    fn a_failed_save_leaves_no_temp_file() {
        let dir = std::env::temp_dir().join(format!("vaegram-write-private-fail-{}", std::process::id()));
        // A directory where the file should go makes the final rename fail.
        let path = dir.join("bot.session");
        std::fs::create_dir_all(path.join("inside")).unwrap();
        let written = write_private(&path, b"key");
        let names: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(written.is_err());
        assert_eq!(names, ["bot.session"]);
    }

    #[test]
    fn create_private_replaces_an_existing_file() {
        let dir = scratch("create-private-replace");
        let path = dir.join("bot.session");
        std::fs::write(&path, b"old contents, longer than the new ones").unwrap();
        let mut file = create_private(&path).unwrap();
        file.write_all(b"new").unwrap();
        drop(file);
        let saved = std::fs::read(&path);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(saved.unwrap(), b"new");
    }

    #[test]
    fn create_private_refuses_a_directory() {
        let dir = scratch("create-private-dir");
        let path = dir.join("bot.session");
        std::fs::create_dir_all(path.join("inside")).unwrap();
        let created = create_private(&path);
        let kept = path.join("inside").is_dir();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(created.unwrap_err().kind(), std::io::ErrorKind::AlreadyExists);
        assert!(kept, "the directory is left alone");
    }

    #[cfg(unix)]
    #[test]
    fn create_private_leaves_a_file_someone_holds_open_alone() {
        use std::io::Read;
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let dir = scratch("create-private-reader");
        let path = dir.join("bot.session");
        std::fs::write(&path, b"old").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let mut reader = std::fs::File::open(&path).unwrap();
        let old_inode = reader.metadata().unwrap().ino();

        let mut file = create_private(&path).unwrap();
        file.write_all(b"secret key").unwrap();
        drop(file);

        let mut seen = Vec::new();
        reader.read_to_end(&mut seen).unwrap();
        let meta = std::fs::metadata(&path).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(seen, b"old", "the reader sees no new bytes");
        assert_ne!(meta.ino(), old_inode, "the new file has its own inode");
        assert_eq!(meta.permissions().mode() & 0o777, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn create_private_replaces_a_symlink_instead_of_following_it() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let dir = scratch("create-private-link");
        let (path, target) = (dir.join("bot.session"), dir.join("target"));
        std::fs::write(&target, b"someone else's file").unwrap();
        symlink(&target, &path).unwrap();

        let mut file = create_private(&path).unwrap();
        file.write_all(b"secret key").unwrap();
        drop(file);

        let meta = std::fs::symlink_metadata(&path).unwrap();
        let (at_path, at_target) = (std::fs::read(&path), std::fs::read(&target));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(at_target.unwrap(), b"someone else's file");
        assert!(meta.is_file(), "path is a regular file now, not a link");
        assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        assert_eq!(at_path.unwrap(), b"secret key");
    }
}
