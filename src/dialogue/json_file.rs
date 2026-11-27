use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use super::{DialogueKey, DialogueStorage, Record};
use crate::storage::StorageFuture;

/// The version of the file's layout. A file of another version is refused, never overwritten.
const VERSION: u32 = 1;

/// Keeps dialogue states in one JSON file, so conversations survive a restart, for any state type with serde's
/// `Serialize` and `Deserialize`. With the `dialogue-json` feature.
///
/// The file is read on first use and rewritten whole on every change, through a fresh temporary file that only the
/// owner can read, synced before it replaces the old one; that suits a bot with up to a few thousand open
/// conversations. The storage caches the whole file, so two of them on one file overwrite each other's changes:
/// to serve several dialogues from one file, make one storage and share it, giving each
/// [`DialogueRouter`](super::DialogueRouter) an `Arc::clone` of the same `Arc<JsonFileStorage>` under its own
/// namespace, and never point two processes at the file. A state the type can no longer read (a variant renamed or
/// removed) reads as no state, so that conversation starts over. Records that timed out are dropped on the next
/// write.
pub struct JsonFileStorage {
    path: PathBuf,
    entries: Mutex<Option<HashMap<String, Entry>>>,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    state: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_at_ms: Option<u64>,
}

#[derive(Deserialize)]
struct FileData {
    version: u32,
    entries: HashMap<String, Entry>,
}

#[derive(Serialize)]
struct FileRef<'a> {
    version: u32,
    entries: &'a HashMap<String, Entry>,
}

impl JsonFileStorage {
    /// A storage in the file at `path`, made on the first change if it doesn't exist.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        JsonFileStorage { path: path.into(), entries: Mutex::new(None) }
    }

    /// The file's path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl std::fmt::Debug for JsonFileStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JsonFileStorage").field("path", &self.path).finish_non_exhaustive()
    }
}

impl<S: Serialize + DeserializeOwned + Send + 'static> DialogueStorage<S> for JsonFileStorage {
    fn get<'a>(&'a self, key: &'a DialogueKey) -> StorageFuture<'a, Option<Record<S>>> {
        Box::pin(async move {
            let mut entries = self.entries.lock().await;
            let map = loaded(&self.path, &mut entries).await?;
            let Some(entry) = map.get(&key.to_string()) else { return Ok(None) };
            let Ok(state) = S::deserialize(&entry.state) else { return Ok(None) };
            Ok(Some(Record::new(state).with_expiry(entry.expires_at_ms.and_then(from_ms))))
        })
    }

    fn set<'a>(&'a self, key: &'a DialogueKey, record: Record<S>) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let state = serde_json::to_value(&record.state).map_err(io::Error::other)?;
            let entry = Entry { state, expires_at_ms: record.expires_at.map(to_ms) };
            let mut entries = self.entries.lock().await;
            let map = loaded(&self.path, &mut entries).await?;
            let now = to_ms(SystemTime::now());
            map.retain(|_, kept| kept.expires_at_ms.is_none_or(|at| at > now));
            map.insert(key.to_string(), entry);
            let written = write(&self.path, map).await;
            if written.is_err() {
                // The file still holds the old records: read it again rather than trust what it doesn't hold.
                *entries = None;
            }
            written
        })
    }

    fn remove<'a>(&'a self, key: &'a DialogueKey) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let mut entries = self.entries.lock().await;
            let map = loaded(&self.path, &mut entries).await?;
            if map.remove(&key.to_string()).is_none() {
                return Ok(());
            }
            let written = write(&self.path, map).await;
            if written.is_err() {
                *entries = None;
            }
            written
        })
    }
}

/// The records, read from `path` the first time.
async fn loaded<'g>(
    path: &Path,
    slot: &'g mut Option<HashMap<String, Entry>>,
) -> io::Result<&'g mut HashMap<String, Entry>> {
    if slot.is_none() {
        *slot = Some(read(path).await?);
    }
    Ok(slot.get_or_insert_with(HashMap::new))
}

async fn read(path: &Path) -> io::Result<HashMap<String, Entry>> {
    let bytes = match tokio::fs::read(path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(error) => return Err(error),
    };
    let data: FileData =
        serde_json::from_slice(&bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if data.version != VERSION {
        let message = format!("dialogue file version {} isn't {VERSION}", data.version);
        return Err(io::Error::new(io::ErrorKind::InvalidData, message));
    }
    Ok(data.entries)
}

/// Replaces the file with `entries`, as the session files are written: a fresh owner-only temporary file under a
/// random name, synced, then renamed over it, off the runtime's worker threads.
async fn write(path: &Path, entries: &HashMap<String, Entry>) -> io::Result<()> {
    let bytes = serde_json::to_vec(&FileRef { version: VERSION, entries }).map_err(io::Error::other)?;
    let path = path.to_owned();
    let written = tokio::task::spawn_blocking(move || crate::storage::write_private(&path, &bytes));
    written.await.map_err(io::Error::other)?
}

fn to_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |since| u64::try_from(since.as_millis()).unwrap_or(u64::MAX))
}

fn from_ms(ms: u64) -> Option<SystemTime> {
    UNIX_EPOCH.checked_add(Duration::from_millis(ms))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    enum Signup {
        Name,
        Age { name: String },
    }

    struct TempFile(PathBuf);

    impl TempFile {
        fn new(name: &str) -> Self {
            let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
            let file = format!("vaegram-dialogue-{name}-{}-{nanos}.json", std::process::id());
            TempFile(std::env::temp_dir().join(file))
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn key(user: i64) -> DialogueKey {
        DialogueKey::new("signup", 5, user)
    }

    #[tokio::test]
    async fn states_survive_a_new_storage_on_the_same_file() {
        let file = TempFile::new("survive");
        let first = JsonFileStorage::new(&file.0);
        let age = Signup::Age { name: "Ann".into() };
        let expiry = from_ms(to_ms(SystemTime::now() + Duration::from_secs(600)));
        first.set(&key(1), Record::new(age.clone()).with_expiry(expiry)).await.unwrap();
        first.set(&key(2), Record::new(Signup::Name)).await.unwrap();
        DialogueStorage::<Signup>::remove(&first, &key(2)).await.unwrap();

        let second = JsonFileStorage::new(&file.0);
        let got = DialogueStorage::<Signup>::get(&second, &key(1)).await.unwrap();
        assert_eq!(got, Some(Record::new(age).with_expiry(expiry)));
        assert_eq!(DialogueStorage::<Signup>::get(&second, &key(2)).await.unwrap(), None);
    }

    #[tokio::test]
    async fn a_state_of_another_shape_reads_as_none() {
        let file = TempFile::new("shape");
        let storage = JsonFileStorage::new(&file.0);
        storage.set(&key(1), Record::new("not a signup".to_owned())).await.unwrap();
        let got = DialogueStorage::<Signup>::get(&storage, &key(1)).await.unwrap();
        assert_eq!(got, None);
    }

    #[tokio::test]
    async fn a_foreign_file_is_refused_and_kept() {
        let file = TempFile::new("foreign");
        std::fs::write(&file.0, b"{\"version\": 9, \"entries\": {}}").unwrap();
        let storage = JsonFileStorage::new(&file.0);
        let error = storage.set(&key(1), Record::new(Signup::Name)).await.unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(std::fs::read(&file.0).unwrap(), b"{\"version\": 9, \"entries\": {}}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_file_is_only_the_owners() {
        use std::os::unix::fs::PermissionsExt;

        let file = TempFile::new("mode");
        let storage = JsonFileStorage::new(&file.0);
        storage.set(&key(1), Record::new(Signup::Name)).await.unwrap();
        let mode = std::fs::metadata(&file.0).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "{mode:o}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_link_left_at_the_temporary_name_is_not_followed() {
        let file = TempFile::new("link");
        let victim = TempFile::new("victim");
        std::fs::write(&victim.0, b"keep me").unwrap();
        let mut guessed = file.0.as_os_str().to_owned();
        guessed.push(".tmp");
        let guessed = TempFile(PathBuf::from(guessed));
        std::os::unix::fs::symlink(&victim.0, &guessed.0).unwrap();

        let storage = JsonFileStorage::new(&file.0);
        storage.set(&key(1), Record::new(Signup::Name)).await.unwrap();
        assert_eq!(std::fs::read(&victim.0).unwrap(), b"keep me");
        let got = DialogueStorage::<Signup>::get(&JsonFileStorage::new(&file.0), &key(1)).await.unwrap();
        assert_eq!(got.map(|record| record.state), Some(Signup::Name));
    }
}
