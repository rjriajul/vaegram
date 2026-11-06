use std::fmt;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};

use super::{SessionData, UpdateState};
use crate::session::Salt;

/// What every [`Storage`] method returns: boxed, so `Arc<dyn Storage>` can hold any engine.
pub type StorageFuture<'a, T> = Pin<Box<dyn Future<Output = io::Result<T>> + Send + 'a>>;

/// Where an engine reports trouble it works around: a damaged record it dropped, a write it will retry. The
/// client turns each report into [`ClientEvent::StorageFailed`](crate::enums::ClientEvent::StorageFailed).
pub type StorageReporter = Arc<dyn Fn(io::Error) + Send + Sync>;

/// What the built-in engines keep their reporter in. Replaced by each `set_reporter`, so an engine that moves to
/// another client reports to that one.
#[derive(Default)]
pub(crate) struct Reports(Mutex<Option<StorageReporter>>);

impl Reports {
    pub(crate) fn set(&self, reporter: StorageReporter) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(reporter);
    }

    /// Silent until a reporter is set, as an engine used on its own is.
    pub(crate) fn report(&self, error: io::Error) {
        // Called outside the lock, so a reporter may set another one.
        let reporter = self.0.lock().unwrap_or_else(PoisonError::into_inner).clone();
        if let Some(reporter) = reporter {
            reporter(error);
        }
    }
}

impl fmt::Debug for Reports {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Reports")
    }
}

/// Where a client keeps its login, its update state and the peers it has met.
///
/// Apart from `set_reporter`, a plain call made once before `open`, the client calls an engine from one thread of
/// its own, never from the caller's runtime, so an engine may block or await as it likes. Open is called before
/// anything else; close may be followed by open again.
pub trait Storage: Send + Sync + 'static {
    /// Called once by the client before [`open`](Self::open), with where to report trouble the engine works around
    /// by itself (a damaged record it dropped, a write it will retry). The built-in engines use it; an engine that
    /// doesn't need to can leave this out and stay silent.
    fn set_reporter(&self, reporter: StorageReporter) {
        let _ = reporter;
    }

    /// Connects, creates or migrates.
    fn open(&self) -> StorageFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    /// Flushes and disconnects.
    fn close(&self) -> StorageFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    /// Forgets the session, the state and every peer, as logging out does.
    fn delete(&self) -> StorageFuture<'_, ()>;

    /// The saved login; `None` when there is none yet, which makes the client log in.
    fn load_session(&self) -> StorageFuture<'_, Option<SessionData>>;

    /// Replaces the saved login.
    fn save_session<'a>(&'a self, session: &'a SessionData) -> StorageFuture<'a, ()>;

    /// The saved update state and salt; `None` when there is none. The client drops a state whose
    /// [`owner`](StoredState::owner) isn't the logged-in account, and clears the peers with it.
    fn load_state(&self) -> StorageFuture<'_, Option<StoredState>>;

    /// Replaces the saved update state and salt.
    fn save_state<'a>(&'a self, state: &'a StoredState) -> StorageFuture<'a, ()>;

    /// Inserts or replaces each peer whole, usernames and phone number included.
    fn update_peers<'a>(&'a self, peers: &'a [PeerRecord]) -> StorageFuture<'a, ()>;

    /// The peer with this Bot-API-style id; `None` when it isn't stored.
    fn get_peer_by_id(&self, id: i64) -> StorageFuture<'_, Option<PeerRecord>>;

    /// `username` is lowercase, as [`PeerRecord::usernames`] are.
    fn get_peer_by_username<'a>(&'a self, username: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        let _ = username;
        Box::pin(async { Ok(None) })
    }

    /// The user with this phone number, as Telegram gives it; `None` when it isn't stored. By default nothing
    /// is found.
    fn get_peer_by_phone_number<'a>(&'a self, phone_number: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        let _ = phone_number;
        Box::pin(async { Ok(None) })
    }

    /// The most recently updated peers first, at most `limit` of them.
    fn export_peers(&self, limit: Option<usize>) -> StorageFuture<'_, Vec<PeerRecord>> {
        let _ = limit;
        Box::pin(async { Ok(Vec::new()) })
    }

    /// Forgets every peer, for a login that belongs to another account.
    fn clear_peers(&self) -> StorageFuture<'_, ()>;
}

impl<T: Storage + ?Sized> Storage for Arc<T> {
    fn set_reporter(&self, reporter: StorageReporter) {
        (**self).set_reporter(reporter)
    }

    fn open(&self) -> StorageFuture<'_, ()> {
        (**self).open()
    }

    fn close(&self) -> StorageFuture<'_, ()> {
        (**self).close()
    }

    fn delete(&self) -> StorageFuture<'_, ()> {
        (**self).delete()
    }

    fn load_session(&self) -> StorageFuture<'_, Option<SessionData>> {
        (**self).load_session()
    }

    fn save_session<'a>(&'a self, session: &'a SessionData) -> StorageFuture<'a, ()> {
        (**self).save_session(session)
    }

    fn load_state(&self) -> StorageFuture<'_, Option<StoredState>> {
        (**self).load_state()
    }

    fn save_state<'a>(&'a self, state: &'a StoredState) -> StorageFuture<'a, ()> {
        (**self).save_state(state)
    }

    fn update_peers<'a>(&'a self, peers: &'a [PeerRecord]) -> StorageFuture<'a, ()> {
        (**self).update_peers(peers)
    }

    fn get_peer_by_id(&self, id: i64) -> StorageFuture<'_, Option<PeerRecord>> {
        (**self).get_peer_by_id(id)
    }

    fn get_peer_by_username<'a>(&'a self, username: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        (**self).get_peer_by_username(username)
    }

    fn get_peer_by_phone_number<'a>(&'a self, phone_number: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        (**self).get_peer_by_phone_number(phone_number)
    }

    fn export_peers(&self, limit: Option<usize>) -> StorageFuture<'_, Vec<PeerRecord>> {
        (**self).export_peers(limit)
    }

    fn clear_peers(&self) -> StorageFuture<'_, ()> {
        (**self).clear_peers()
    }
}

/// A user or channel the client can reach again without looking it up.
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PeerRecord {
    /// Bot API style: users positive, channels `-100…`.
    pub id: i64,
    /// The access hash that lets this account reach the peer.
    pub access_hash: i64,
    /// Active usernames, lowercase.
    pub usernames: Vec<String>,
    /// The user's phone number as Telegram gave it, when the account may see it.
    pub phone_number: Option<String>,
    /// Unix time of the last change.
    pub updated: i64,
}

impl PeerRecord {
    /// A peer with no usernames, no phone number and `updated` 0.
    pub fn new(id: i64, access_hash: i64) -> PeerRecord {
        PeerRecord { id, access_hash, ..PeerRecord::default() }
    }
}

/// The account the state belongs to, the server salt and where updates resume.
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StoredState {
    /// The user id of the account the state belongs to; 0 before login.
    pub owner: i64,
    /// The server salt and clock offset to start the next connection with.
    pub salt: Salt,
    /// Where updates resume; `None` before the first update state is known.
    pub update: Option<UpdateState>,
}

impl StoredState {
    /// A state from its parts.
    pub fn new(owner: i64, salt: Salt, update: Option<UpdateState>) -> StoredState {
        StoredState { owner, salt, update }
    }
}
