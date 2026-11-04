use std::fmt;
use std::io;
use std::sync::Arc;
use std::time::Duration;

use object_store::aws::AmazonS3Builder;
use object_store::path::Path;
use object_store::{ObjectStore, ObjectStoreExt, PutMode, PutOptions, PutPayload, UpdateVersion};
use tokio::sync::{Mutex, MutexGuard};
use tokio::time::Instant;

use super::SessionData;
use super::engine::{PeerRecord, Storage, StorageFuture, StoredState};
use super::file_format;
use super::memory_storage::Tables;

const INTERVAL: Duration = Duration::from_secs(30);

const CHANGED: &str = "the object changed since vaegram read it (another client writes it); it was left as it is";
const UNSUPPORTED: &str = "this S3 store doesn't support conditional writes; vaegram won't overwrite blindly";

/// Keeps everything in one S3 object, in the format of a [`FileStorage`](super::FileStorage) file, so a copy of
/// the object opens as a session file and the other way round.
///
/// Every record is held in memory. A new login is uploaded at once; state and peer changes are uploaded at most
/// every 30 seconds, with the next change after that, and on close. Each upload replaces the object whole, so a
/// failed one leaves the last good object, and the next upload carries everything since. An object vaegram
/// didn't write is refused and left as it is.
///
/// Every upload is conditional: it names the version of the object vaegram last read or wrote (`If-Match`), or
/// asks that there be none yet (`If-None-Match: *`). If another client wrote the object in between, the upload
/// fails, the object keeps the other client's content, and this storage writes nothing more until it is opened
/// again. The store must honour those headers on PUT, as AWS S3, Cloudflare R2 and MinIO do; one that ignores them
/// would overwrite without the check. With `AWS_CONDITIONAL_PUT=disabled` every upload is refused rather than made
/// unconditional.
///
/// Without `AWS_*` credentials in the environment, the S3 client asks the EC2 instance-metadata endpoint for them,
/// so off AWS opening can take a while before it fails.
pub struct S3Storage {
    store: Arc<dyn ObjectStore>,
    key: Path,
    interval: Duration,
    object: Mutex<Object>,
    #[cfg(test)]
    fail_uploads: std::sync::atomic::AtomicBool,
}

#[derive(Default)]
struct Object {
    opened: bool,
    tables: Tables,
    // Whole records of kinds this version skips, kept for a newer vaegram.
    skipped: Vec<u8>,
    dirty: bool,
    uploaded: Option<Instant>,
    // The version uploads must replace; None while there is no object.
    version: Option<UpdateVersion>,
    // Set when another client wrote the object: nothing is uploaded over it until the next open.
    conflict: bool,
}

impl S3Storage {
    /// The object `key` in `bucket`. Credentials, the region and the endpoint come from the `AWS_*` environment
    /// variables. Nothing is fetched until the storage is opened.
    pub fn from_env(bucket: &str, key: &str) -> io::Result<S3Storage> {
        S3Storage::build(AmazonS3Builder::from_env().with_bucket_name(bucket), key)
    }

    fn build(builder: AmazonS3Builder, key: &str) -> io::Result<S3Storage> {
        let key = Path::parse(key).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "not an S3 object key"))?;
        let store = builder.build().map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("s3: {e}")))?;
        Ok(S3Storage::with_store(Arc::new(store), key))
    }

    fn with_store(store: Arc<dyn ObjectStore>, key: Path) -> S3Storage {
        S3Storage {
            store,
            key,
            interval: INTERVAL,
            object: Mutex::new(Object::default()),
            #[cfg(test)]
            fail_uploads: std::sync::atomic::AtomicBool::new(false),
        }
    }

    fn named(&self, e: io::Error) -> io::Error {
        io::Error::new(e.kind(), format!("{}: {e}", self.key))
    }

    /// The object's records, downloaded first if they haven't been. Only a missing object starts empty: a failed
    /// download is an error, so nothing is ever uploaded over an object that couldn't be read.
    async fn object(&self) -> io::Result<MutexGuard<'_, Object>> {
        let mut object = self.object.lock().await;
        if !object.opened {
            self.download(&mut object).await.map_err(|e| self.named(e))?;
        }
        Ok(object)
    }

    async fn download(&self, object: &mut Object) -> io::Result<()> {
        let mut version = None;
        let bytes = match self.store.get(&self.key).await {
            Ok(got) => {
                version = Some(UpdateVersion { e_tag: got.meta.e_tag.clone(), version: got.meta.version.clone() });
                Some(got.bytes().await.map_err(error)?)
            }
            Err(object_store::Error::NotFound { .. }) => None,
            Err(e) => return Err(error(e)),
        };
        let replayed = file_format::replay(bytes.as_deref().unwrap_or_default())?;
        if replayed.damaged {
            eprintln!("vaegram: {}: dropping a half-written record; the next upload leaves it out", self.key);
        }
        *object = Object {
            opened: true,
            tables: replayed.tables,
            skipped: replayed.skipped,
            dirty: replayed.damaged,
            uploaded: None,
            version,
            conflict: false,
        };
        Ok(())
    }

    async fn upload(&self, object: &mut Object) -> io::Result<()> {
        if object.conflict {
            return Err(io::Error::new(io::ErrorKind::InvalidData, CHANGED));
        }
        let mode = match &object.version {
            None => PutMode::Create,
            Some(v) if v.e_tag.is_some() => PutMode::Update(v.clone()),
            Some(_) => return Err(io::Error::new(io::ErrorKind::Unsupported, UNSUPPORTED)),
        };
        let bytes = file_format::snapshot(&object.tables, &object.skipped);
        #[cfg(test)]
        if self.fail_uploads.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(io::Error::other("the upload failed"));
        }
        match self.store.put_opts(&self.key, PutPayload::from(bytes), PutOptions::from(mode)).await {
            Ok(put) => {
                object.version = Some(UpdateVersion { e_tag: put.e_tag, version: put.version });
                (object.dirty, object.uploaded) = (false, Some(Instant::now()));
                Ok(())
            }
            Err(object_store::Error::Precondition { .. } | object_store::Error::AlreadyExists { .. }) => {
                object.conflict = true;
                Err(io::Error::new(io::ErrorKind::InvalidData, CHANGED))
            }
            Err(object_store::Error::NotImplemented { .. } | object_store::Error::NotSupported { .. }) => {
                Err(io::Error::new(io::ErrorKind::Unsupported, UNSUPPORTED))
            }
            Err(e) => Err(error(e)),
        }
    }

    /// Notes a change and uploads it if `now` asks for it or the last upload is older than the interval.
    async fn changed(&self, object: &mut Object, now: bool) -> io::Result<()> {
        object.dirty = true;
        if now || object.conflict || object.uploaded.is_none_or(|at| at.elapsed() >= self.interval) {
            self.upload(object).await.map_err(|e| self.named(e))?;
        }
        Ok(())
    }
}

fn error(e: object_store::Error) -> io::Error {
    let kind = match &e {
        object_store::Error::NotFound { .. } => io::ErrorKind::NotFound,
        object_store::Error::PermissionDenied { .. } | object_store::Error::Unauthenticated { .. } => {
            io::ErrorKind::PermissionDenied
        }
        _ => io::ErrorKind::Other,
    };
    io::Error::new(kind, format!("s3: {e}"))
}

impl fmt::Debug for S3Storage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("S3Storage").field("key", &self.key.as_ref()).finish_non_exhaustive()
    }
}

impl Storage for S3Storage {
    fn open(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move { self.object().await.map(drop) })
    }

    fn close(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let mut object = self.object.lock().await;
            if object.conflict {
                // The other client's object wins; what this one changed since is dropped.
                *object = Object::default();
                return Err(self.named(io::Error::new(io::ErrorKind::InvalidData, CHANGED)));
            }
            if object.opened && object.dirty {
                self.upload(&mut object).await.map_err(|e| self.named(e))?;
            }
            *object = Object::default();
            Ok(())
        })
    }

    fn delete(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let mut object = self.object.lock().await;
            match self.store.delete(&self.key).await {
                Ok(()) | Err(object_store::Error::NotFound { .. }) => {}
                Err(e) => return Err(self.named(error(e))),
            }
            *object = Object { opened: true, ..Object::default() };
            Ok(())
        })
    }

    fn load_session(&self) -> StorageFuture<'_, Option<SessionData>> {
        Box::pin(async move { Ok(self.object().await?.tables.session.clone()) })
    }

    fn save_session<'a>(&'a self, session: &'a SessionData) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let mut object = self.object().await?;
            object.tables.session = Some(session.clone());
            // A login lost to a crash means logging in again, so it doesn't wait for the interval.
            self.changed(&mut object, true).await
        })
    }

    fn load_state(&self) -> StorageFuture<'_, Option<StoredState>> {
        Box::pin(async move { Ok(self.object().await?.tables.state.clone()) })
    }

    fn save_state<'a>(&'a self, state: &'a StoredState) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let mut object = self.object().await?;
            object.tables.state = Some(state.clone());
            self.changed(&mut object, false).await
        })
    }

    fn update_peers<'a>(&'a self, peers: &'a [PeerRecord]) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let mut object = self.object().await?;
            if peers.is_empty() {
                return Ok(());
            }
            for p in peers {
                object.tables.upsert(p.clone());
            }
            self.changed(&mut object, false).await
        })
    }

    fn get_peer_by_id(&self, id: i64) -> StorageFuture<'_, Option<PeerRecord>> {
        Box::pin(async move { Ok(self.object().await?.tables.by_id(id)) })
    }

    fn get_peer_by_username<'a>(&'a self, username: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        Box::pin(async move { Ok(self.object().await?.tables.by_username(username)) })
    }

    fn get_peer_by_phone_number<'a>(&'a self, phone_number: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        Box::pin(async move { Ok(self.object().await?.tables.by_phone_number(phone_number)) })
    }

    fn export_peers(&self, limit: Option<usize>) -> StorageFuture<'_, Vec<PeerRecord>> {
        Box::pin(async move { Ok(self.object().await?.tables.export(limit)) })
    }

    fn clear_peers(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let mut object = self.object().await?;
            object.tables.clear_peers();
            self.changed(&mut object, false).await
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::Arc;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use object_store::aws::{AmazonS3Builder, S3ConditionalPut};
    use object_store::memory::InMemory;
    use object_store::path::Path;
    use object_store::{ObjectStoreExt, PutPayload, RetryConfig, UpdateVersion};

    use super::S3Storage;
    use crate::session::Salt;
    use crate::storage::{PeerRecord, SessionData, Storage, StoredState, conformance, file_format};

    const KEY: &str = "bots/bot.session";

    fn in_memory() -> (Arc<InMemory>, S3Storage) {
        let store = Arc::new(InMemory::new());
        (store.clone(), S3Storage::with_store(store, Path::from(KEY)))
    }

    async fn object(store: &InMemory) -> Option<Vec<u8>> {
        match store.get(&Path::from(KEY)).await {
            Ok(got) => Some(got.bytes().await.unwrap().to_vec()),
            Err(object_store::Error::NotFound { .. }) => None,
            Err(e) => panic!("{e}"),
        }
    }

    fn session(user_id: i64) -> SessionData {
        let mut s = SessionData::new(4, [9; 256]);
        s.user_id = user_id;
        s
    }

    fn peer(id: i64, updated: i64) -> PeerRecord {
        let mut p = PeerRecord::new(id, id * 3);
        (p.usernames, p.updated) = (vec![format!("user{id}")], updated);
        p
    }

    #[tokio::test]
    async fn conformance_on_an_in_memory_store() {
        let (store, storage) = in_memory();
        conformance::check(&storage).await;
        assert_eq!(object(&store).await, None, "delete removes the object");
    }

    #[tokio::test]
    async fn the_object_is_a_session_file() {
        let (store, storage) = in_memory();
        storage.save_session(&session(77)).await.unwrap();
        storage.update_peers(&[peer(1, 5), peer(2, 6)]).await.unwrap();
        let state = StoredState::new(77, Salt { value: 1, time_offset: 2 }, None);
        storage.save_state(&state).await.unwrap();
        storage.close().await.unwrap();

        let replayed = file_format::replay(&object(&store).await.unwrap()).unwrap();
        assert!(!replayed.damaged);
        assert_eq!(replayed.tables.session.as_ref().map(|s| s.user_id), Some(77));
        assert_eq!(replayed.tables.state, Some(state));
        assert_eq!(replayed.tables.export(None), [peer(2, 6), peer(1, 5)]);
    }

    #[tokio::test(start_paused = true)]
    async fn changes_wait_for_the_interval_or_close() {
        let (store, storage) = in_memory();
        storage.save_state(&StoredState::new(1, Salt::default(), None)).await.unwrap();
        let first = object(&store).await.expect("the first change is uploaded");
        storage.update_peers(&[peer(1, 1)]).await.unwrap();
        assert_eq!(object(&store).await, Some(first.clone()), "a change within the interval waits");

        tokio::time::advance(Duration::from_secs(31)).await;
        storage.update_peers(&[peer(2, 2)]).await.unwrap();
        let replayed = file_format::replay(&object(&store).await.unwrap()).unwrap();
        assert_eq!(replayed.tables.export(None), [peer(2, 2), peer(1, 1)], "both peers went up together");

        storage.update_peers(&[peer(3, 3)]).await.unwrap();
        storage.save_session(&session(5)).await.unwrap();
        let replayed = file_format::replay(&object(&store).await.unwrap()).unwrap();
        assert_eq!(replayed.tables.export(None).len(), 3, "a new login goes up at once, with what waited");

        storage.clear_peers().await.unwrap();
        storage.close().await.unwrap();
        let replayed = file_format::replay(&object(&store).await.unwrap()).unwrap();
        assert!(replayed.tables.export(None).is_empty(), "close uploads what waited");
    }

    #[tokio::test]
    async fn a_failed_upload_keeps_the_last_object_and_retries() {
        let (store, storage) = in_memory();
        storage.save_session(&session(1)).await.unwrap();
        let good = object(&store).await.unwrap();

        storage.fail_uploads.store(true, Ordering::Relaxed);
        assert!(storage.save_session(&session(2)).await.is_err());
        assert!(storage.close().await.is_err(), "close reports the change it couldn't upload");
        assert_eq!(object(&store).await, Some(good), "the last good object stays whole");

        storage.fail_uploads.store(false, Ordering::Relaxed);
        storage.close().await.unwrap();
        let replayed = file_format::replay(&object(&store).await.unwrap()).unwrap();
        assert_eq!(replayed.tables.session.as_ref().map(|s| s.user_id), Some(2), "the next upload carries the change");
        assert_eq!(storage.load_session().await.unwrap().map(|s| s.user_id), Some(2));
    }

    #[tokio::test]
    async fn refuses_objects_it_did_not_write_untouched() {
        let mut newer = file_format::snapshot(&Default::default(), &[]);
        newer[16] = 2;
        let cases = [(b"VAE_AQIDBA".to_vec(), "not a vaegram session file"), (newer, "newer vaegram")];
        for (bytes, why) in cases {
            let (store, storage) = in_memory();
            store.put(&Path::from(KEY), PutPayload::from(bytes.clone())).await.unwrap();
            let error = storage.open().await.unwrap_err();
            let saved = storage.save_session(&session(1)).await;
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            assert!(error.to_string().contains(why) && error.to_string().contains(KEY), "{error}");
            assert!(saved.is_err());
            assert_eq!(object(&store).await, Some(bytes), "the object was left as it is");
        }
    }

    #[tokio::test]
    async fn another_writer_is_never_overwritten() {
        let (store, first) = in_memory();
        first.save_session(&session(1)).await.unwrap();
        let second = S3Storage::with_store(store.clone(), Path::from(KEY));
        second.save_session(&session(2)).await.unwrap();
        let theirs = object(&store).await.unwrap();

        let error = first.save_session(&session(3)).await.unwrap_err();
        assert!(error.to_string().contains("changed since vaegram read it"), "{error}");
        assert!(first.update_peers(&[peer(1, 1)]).await.is_err(), "nothing more is uploaded over it");
        assert!(first.close().await.is_err());
        assert_eq!(object(&store).await, Some(theirs.clone()), "the other client's object stays");

        let created = S3Storage::with_store(store.clone(), Path::from(KEY));
        created.object.lock().await.opened = true;
        assert!(created.save_session(&session(4)).await.is_err(), "creating over an existing object fails");
        assert_eq!(object(&store).await, Some(theirs));
        assert_eq!(first.load_session().await.unwrap().map(|s| s.user_id), Some(2), "open reads it again");
    }

    #[tokio::test]
    async fn a_store_without_conditional_puts_is_never_written() {
        let storage = S3Storage::build(unreachable().with_conditional_put(S3ConditionalPut::Disabled), KEY).unwrap();
        // Skip the download: the store refuses the conditional put before sending anything.
        storage.object.lock().await.opened = true;
        let created = storage.save_session(&session(1)).await.unwrap_err();
        storage.object.lock().await.version = Some(UpdateVersion { e_tag: Some("\"1\"".into()), version: None });
        let updated = storage.save_session(&session(1)).await.unwrap_err();
        for error in [created, updated] {
            assert_eq!(error.kind(), io::ErrorKind::Unsupported);
            assert!(error.to_string().contains("doesn't support conditional writes"), "{error}");
        }
        storage.object.lock().await.version = Some(UpdateVersion { e_tag: None, version: Some("v1".into()) });
        let untagged = storage.save_session(&session(1)).await.unwrap_err();
        assert_eq!(untagged.kind(), io::ErrorKind::Unsupported, "no ETag to name: no write");
    }

    fn unreachable() -> AmazonS3Builder {
        let retry = RetryConfig { max_retries: 0, retry_timeout: Duration::from_secs(5), ..RetryConfig::default() };
        AmazonS3Builder::new()
            .with_bucket_name("bucket")
            .with_region("us-east-1")
            .with_access_key_id("AKIDEXAMPLE")
            .with_secret_access_key("hunter2")
            .with_endpoint("http://127.0.0.1:1")
            .with_allow_http(true)
            .with_retry(retry)
    }

    #[tokio::test]
    async fn an_unreachable_store_is_an_error_and_hides_the_secret() {
        let storage = S3Storage::build(unreachable(), KEY).unwrap();
        let debug = format!("{storage:?}");
        let error = storage.open().await.unwrap_err().to_string();
        let saved = storage.save_session(&session(1)).await;
        assert_eq!(debug, r#"S3Storage { key: "bots/bot.session", .. }"#);
        assert!(!error.contains("hunter2") && !error.contains("AKIDEXAMPLE"), "{error}");
        assert!(saved.is_err(), "nothing is uploaded over an object that couldn't be read");
        assert!(S3Storage::build(unreachable(), "a//b").is_err());
    }

    #[tokio::test]
    #[ignore = "needs VAEGRAM_S3_BUCKET and AWS_* credentials for a bucket it may write to"]
    async fn conformance_on_a_live_bucket() {
        let Ok(bucket) = std::env::var("VAEGRAM_S3_BUCKET") else {
            assert!(std::env::var_os("CI").is_none(), "set VAEGRAM_S3_BUCKET");
            return;
        };
        let key = format!("vaegram-live-{}.session", std::process::id());
        conformance::check(&S3Storage::from_env(&bucket, &key).unwrap()).await;
    }
}
