use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use super::state::{self, Peers, UpdateState};
use super::{SessionData, session_string};
use crate::session::Salt;

const SESSION: &str = "session";
const STATE: &str = "state";

/// Where a client keeps what it saves between runs, as byte values under names.
///
/// vaegram uses two names. `"session"` holds the login as a session string, the same text
/// [`export_session_string`](crate::Client::export_session_string) returns. `"state"` holds known chats, the update
/// state and the server salt in vaegram's own versioned format. Later versions may add names, so a store should keep
/// whatever it is given.
///
/// The `"session"` value is the account's auth key: anyone who can read it is logged in as that account. A custom
/// store has to protect it itself, by access control, encryption at rest or both.
///
/// Calls block. State saves run on tokio's blocking threads, but loads, session saves (on connect, data center
/// switch and sign-in) and the last state save, made when the client is dropped, run on the calling thread.
pub trait SessionStore: Send + Sync + 'static {
    /// The value last saved under `key`, or `None` when there is none.
    fn load(&self, key: &str) -> io::Result<Option<Vec<u8>>>;

    /// Replaces the value under `key`, whole or not at all: a value cut short reads back as damaged.
    fn save(&self, key: &str, bytes: &[u8]) -> io::Result<()>;

    /// Removes `key`; a key that isn't there is not an error.
    fn delete(&self, key: &str) -> io::Result<()>;
}

/// Lets the caller keep a handle to the store it gives the client.
impl<T: SessionStore + ?Sized> SessionStore for Arc<T> {
    fn load(&self, key: &str) -> io::Result<Option<Vec<u8>>> {
        (**self).load(key)
    }

    fn save(&self, key: &str, bytes: &[u8]) -> io::Result<()> {
        (**self).save(key, bytes)
    }

    fn delete(&self, key: &str) -> io::Result<()> {
        (**self).delete(key)
    }
}

/// The default store: `<name>.session` and `<name>.state`, each replaced atomically and, on Unix, readable by its
/// owner only.
#[derive(Clone, Debug)]
pub struct FileStore {
    session: PathBuf,
}

impl FileStore {
    /// `name` is what [`Client::builder`](crate::Client::builder) takes, so it can hold a directory:
    /// `"bots/echo"` keeps `bots/echo.session` and `bots/echo.state`.
    pub fn new(name: impl AsRef<Path>) -> FileStore {
        let mut session = name.as_ref().as_os_str().to_owned();
        session.push(".session");
        FileStore { session: PathBuf::from(session) }
    }

    fn path(&self, key: &str) -> Cow<'_, Path> {
        match key {
            SESSION => Cow::Borrowed(&self.session),
            _ => Cow::Owned(self.session.with_extension(key)),
        }
    }
}

fn naming(path: &Path) -> impl FnOnce(io::Error) -> io::Error + '_ {
    move |e| io::Error::new(e.kind(), format!("{}: {e}", path.display()))
}

impl SessionStore for FileStore {
    fn load(&self, key: &str) -> io::Result<Option<Vec<u8>>> {
        let path = self.path(key);
        match fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(naming(&path)(e)),
        }
    }

    fn save(&self, key: &str, bytes: &[u8]) -> io::Result<()> {
        let path = self.path(key);
        super::write_private(&path, bytes).map_err(naming(&path))
    }

    fn delete(&self, key: &str) -> io::Result<()> {
        let path = self.path(key);
        super::delete(&path).map_err(naming(&path))
    }
}

/// Keeps everything in memory, so nothing outlives the process. A client started from a session string without a
/// store uses one.
#[derive(Default)]
pub struct MemoryStore(Mutex<HashMap<String, Vec<u8>>>);

// The values hold the auth key, so they stay out of logs.
impl fmt::Debug for MemoryStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryStore").finish_non_exhaustive()
    }
}

impl SessionStore for MemoryStore {
    fn load(&self, key: &str) -> io::Result<Option<Vec<u8>>> {
        Ok(self.0.lock().unwrap_or_else(PoisonError::into_inner).get(key).cloned())
    }

    fn save(&self, key: &str, bytes: &[u8]) -> io::Result<()> {
        let mut values = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        match values.get_mut(key) {
            Some(value) => {
                value.clear();
                value.extend_from_slice(bytes);
            }
            None => {
                values.insert(key.to_owned(), bytes.to_vec());
            }
        }
        Ok(())
    }

    fn delete(&self, key: &str) -> io::Result<()> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).remove(key);
        Ok(())
    }
}

/// A client's store, with the ordering vaegram needs on top: saves come from several threads and a slow backend
/// must not let an older snapshot land last.
pub(crate) struct Store<S: ?Sized = dyn SessionStore> {
    encoded: AtomicU64,
    // The newest state ticket saved. u64::MAX after log_out, which also stops session saves; both kinds take this
    // lock, so a save already waiting can't write the dead login back.
    written: Mutex<u64>,
    backend: S,
}

impl<S: SessionStore> Store<S> {
    pub(crate) fn new(backend: S) -> Self {
        Store { encoded: AtomicU64::new(0), written: Mutex::new(0), backend }
    }
}

impl<S: SessionStore + ?Sized> Store<S> {
    pub(crate) fn load_session(&self) -> io::Result<Option<SessionData>> {
        let Some(bytes) = self.backend.load(SESSION)? else { return Ok(None) };
        session_string::parse(&String::from_utf8_lossy(&bytes))
            .map(Some)
            .map_err(|why| io::Error::new(io::ErrorKind::InvalidData, format!("stored session: {why}")))
    }

    pub(crate) fn save_session(&self, data: &SessionData) -> io::Result<()> {
        let written = self.written.lock().unwrap_or_else(PoisonError::into_inner);
        if *written == u64::MAX {
            return Ok(());
        }
        self.backend.save(SESSION, session_string::export(data).as_bytes())
    }

    pub(crate) fn load_state(&self, user_id: i64) -> io::Result<(Peers, Option<UpdateState>, Salt)> {
        let Some(bytes) = self.backend.load(STATE)? else { return Ok(Default::default()) };
        Ok(state::decode(&bytes, user_id).unwrap_or_else(|| {
            eprintln!("vaegram: ignoring the stored state: damaged or from a newer vaegram; it will be overwritten");
            Default::default()
        }))
    }

    /// Taken while the snapshot is encoded, so tickets follow the order of the snapshots.
    pub(crate) fn ticket(&self) -> u64 {
        self.encoded.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub(crate) fn save_state(&self, ticket: u64, bytes: &[u8]) {
        let mut written = self.written.lock().unwrap_or_else(PoisonError::into_inner);
        if ticket < *written {
            return;
        }
        match self.backend.save(STATE, bytes) {
            Ok(()) => *written = ticket,
            Err(e) => eprintln!("vaegram: failed to save the update state: {e}"),
        }
    }

    /// Removes the session and the state for good: no later save, even one already queued, writes them back.
    /// Both are tried; the first error is returned.
    pub(crate) fn delete(&self) -> io::Result<()> {
        *self.written.lock().unwrap_or_else(PoisonError::into_inner) = u64::MAX;
        let session = self.backend.delete(SESSION);
        let state = self.backend.delete(STATE);
        session.and(state)
    }
}

/// Records every call and can be told to fail them all.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct Recording {
    calls: Mutex<Vec<String>>,
    values: MemoryStore,
    fail: bool,
}

#[cfg(test)]
impl Recording {
    pub(crate) fn failing() -> Self {
        Recording { fail: true, ..Recording::default() }
    }

    pub(crate) fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    pub(crate) fn value(&self, key: &str) -> Option<Vec<u8>> {
        self.values.load(key).unwrap()
    }

    fn call(&self, call: &str, key: &str) -> io::Result<()> {
        self.calls.lock().unwrap().push(format!("{call} {key}"));
        if self.fail { Err(io::Error::other("store is down")) } else { Ok(()) }
    }
}

#[cfg(test)]
impl SessionStore for Recording {
    fn load(&self, key: &str) -> io::Result<Option<Vec<u8>>> {
        self.call("load", key)?;
        self.values.load(key)
    }

    fn save(&self, key: &str, bytes: &[u8]) -> io::Result<()> {
        self.call("save", key)?;
        self.values.save(key, bytes)
    }

    fn delete(&self, key: &str) -> io::Result<()> {
        self.call("delete", key)?;
        self.values.delete(key)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{FileStore, MemoryStore, Recording, SessionStore, Store};
    use crate::raw::{base, types};
    use crate::session::Salt;
    use crate::storage::state::{self, Peers, UpdateState};
    use crate::storage::{SessionData, session_string};
    use crate::utils::base64url_encode;

    fn data() -> SessionData {
        SessionData {
            dc_id: 4,
            test_mode: false,
            addr: None,
            api_id: 12345,
            auth_key: [7; 256],
            user_id: 777000,
            is_bot: true,
        }
    }

    fn same(a: &SessionData, b: &SessionData) -> bool {
        let fields = |s: &SessionData| (s.dc_id, s.test_mode, s.addr, s.api_id, s.auth_key, s.user_id, s.is_bot);
        fields(a) == fields(b)
    }

    // The fields every layout shares, as `parse` reads them: dc, api_id, test mode, key, user id, is_bot.
    fn shared_fields() -> Vec<u8> {
        let mut b = vec![4];
        b.extend_from_slice(&12345u32.to_be_bytes());
        b.push(0);
        b.extend_from_slice(&[7; 256]);
        b.extend_from_slice(&777000u64.to_be_bytes());
        b.push(1);
        b
    }

    fn v3_string() -> String {
        let mut b = vec![3];
        b.extend_from_slice(&shared_fields());
        b.extend_from_slice(&0u16.to_be_bytes());
        b.extend_from_slice(&[0; 48]);
        b.extend_from_slice(&crc32fast::hash(&b).to_le_bytes());
        format!("{}{}", session_string::PREFIX, base64url_encode(&b))
    }

    fn peers() -> Peers {
        let user = types::InputPeerUser { user_id: 777000, access_hash: -5 };
        Peers::from([(777000, base::InputPeer::InputPeerUser(Box::new(user)))])
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("vaegram-store-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_file_store_keeps_each_key_in_its_own_file() {
        let dir = temp_dir("files");
        let store = FileStore::new(dir.join("bot"));
        let before = (store.load("session").unwrap(), store.delete("state"));
        store.save("session", b"login").unwrap();
        store.save("state", b"chats").unwrap();
        let files = (std::fs::read(dir.join("bot.session")).ok(), std::fs::read(dir.join("bot.state")).ok());
        let loaded = (store.load("session").unwrap(), store.load("state").unwrap());
        store.delete("session").unwrap();
        store.delete("state").unwrap();
        let left = std::fs::read_dir(&dir).unwrap().count();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(matches!(before, (None, Ok(()))));
        assert_eq!(files, (Some(b"login".to_vec()), Some(b"chats".to_vec())));
        assert_eq!(loaded, files);
        assert_eq!(left, 0);
    }

    #[test]
    fn the_memory_store_replaces_and_forgets_values() {
        let store = MemoryStore::default();
        store.save("session", b"longer value").unwrap();
        store.save("session", b"short").unwrap();
        let saved = store.load("session").unwrap();
        store.delete("session").unwrap();
        assert_eq!(saved.as_deref(), Some(&b"short"[..]));
        assert_eq!(store.load("session").unwrap(), None);
        assert!(store.delete("session").is_ok());
    }

    #[test]
    fn session_and_state_files_written_before_still_load() {
        let dir = temp_dir("old-files");
        let legacy = base64url_encode(&shared_fields());
        let mut v1 =
            state::encode(777000, Salt::default(), &peers(), Some(&UpdateState { date: 1, ..Default::default() }));
        v1.drain(9..25);
        v1[0] = 1;
        std::fs::write(dir.join("bot.state"), &v1).unwrap();
        let store = Store::new(FileStore::new(dir.join("bot")));
        let mut loaded = Vec::new();
        for text in [v3_string(), legacy, session_string::export(&data())] {
            std::fs::write(dir.join("bot.session"), text).unwrap();
            loaded.push(store.load_session());
        }
        let state = store.load_state(777000);
        let _ = std::fs::remove_dir_all(&dir);
        for session in loaded {
            assert!(same(&session.unwrap().unwrap(), &data()));
        }
        let (peers_back, update_state, _) = state.unwrap();
        assert_eq!((peers_back, update_state.map(|s| s.date)), (peers(), Some(1)));
    }

    #[test]
    fn missing_keys_mean_a_fresh_start() {
        let store = Store::new(MemoryStore::default());
        assert!(store.load_session().unwrap().is_none());
        assert_eq!(store.load_state(1).unwrap(), Default::default());
    }

    #[test]
    fn damaged_values_are_errors_or_ignored_never_panics() {
        let store = Store::new(MemoryStore::default());
        store.backend.save("session", &[0xff, 0xfe, b'V']).unwrap();
        store.backend.save("state", &[2, 1]).unwrap();
        let session = store.load_session().map(|s| s.is_some());
        assert_eq!(session.map_err(|e| e.kind()), Err(std::io::ErrorKind::InvalidData));
        assert_eq!(store.load_state(1).unwrap(), Default::default());
    }

    #[test]
    fn store_failures_reach_the_caller() {
        let store = Store::new(Recording::failing());
        assert!(store.load_session().is_err());
        assert!(store.load_state(1).is_err());
        assert!(store.save_session(&data()).is_err());
        store.save_state(store.ticket(), b"state");
        assert!(store.delete().is_err());
        let calls = ["load session", "load state", "save session", "save state", "delete session", "delete state"];
        assert_eq!(store.backend.calls(), calls);
    }

    #[test]
    fn an_older_state_never_replaces_a_newer_one() {
        let store = Store::new(MemoryStore::default());
        let (old, new) = (store.ticket(), store.ticket());
        store.save_state(new, b"new");
        store.save_state(old, b"old");
        assert_eq!(store.backend.load("state").unwrap().as_deref(), Some(&b"new"[..]));
    }

    #[test]
    fn a_failed_state_save_lets_an_older_snapshot_through() {
        let store = Store::new(Arc::new(Recording::failing()));
        let (old, new) = (store.ticket(), store.ticket());
        store.save_state(new, b"new");
        assert_eq!(*store.written.lock().unwrap(), 0, "a failed save records nothing");
        store.save_state(old, b"old");
        assert_eq!(store.backend.calls(), ["save state", "save state"]);
    }

    #[test]
    fn nothing_is_saved_after_delete() {
        let store = Store::new(Recording::default());
        let queued = store.ticket();
        store.delete().unwrap();
        store.save_session(&data()).unwrap();
        store.save_state(queued, b"old account");
        store.save_state(store.ticket(), b"later");
        assert_eq!(store.backend.calls(), ["delete session", "delete state"]);
        assert_eq!((store.backend.value("session"), store.backend.value("state")), (None, None));
    }

    #[test]
    fn file_errors_name_the_file() {
        let dir = temp_dir("named");
        let session = dir.join("bot.session");
        // A directory where the session file should be can't be read, replaced or removed.
        std::fs::create_dir_all(session.join("inside")).unwrap();
        let store = Store::new(FileStore::new(dir.join("bot")));
        let errors = [store.load_session().map(|_| ()), store.save_session(&data()), store.backend.delete("session")];
        let _ = std::fs::remove_dir_all(&dir);
        for error in errors {
            let message = error.unwrap_err().to_string();
            assert!(message.starts_with(&format!("{}: ", session.display())), "{message}");
        }
    }

    #[test]
    fn delete_tries_both_keys_and_returns_the_first_error() {
        let dir = temp_dir("delete");
        // A directory where the session file should be makes its delete fail.
        std::fs::create_dir_all(dir.join("bot.session").join("inside")).unwrap();
        std::fs::write(dir.join("bot.state"), b"old").unwrap();
        let deleted = Store::new(FileStore::new(dir.join("bot"))).delete();
        let state_left = dir.join("bot.state").exists();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(deleted.is_err());
        assert!(!state_left);
    }
}
