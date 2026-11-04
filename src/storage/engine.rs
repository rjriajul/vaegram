use std::io;
use std::pin::Pin;
use std::sync::Arc;

use super::SessionData;
use super::state::UpdateState;
use crate::session::Salt;

/// What every [`Storage`] method returns: boxed, so `Arc<dyn Storage>` can hold any engine.
pub type StorageFuture<'a, T> = Pin<Box<dyn Future<Output = io::Result<T>> + Send + 'a>>;

/// Where a client keeps its login, its update state and the peers it has met.
///
/// The client calls an engine from one thread of its own, never from the caller's runtime, so an engine may block
/// or await as it likes. Open is called before anything else; close may be followed by open again.
pub trait Storage: Send + Sync + 'static {
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

    fn load_session(&self) -> StorageFuture<'_, Option<SessionData>>;

    fn save_session<'a>(&'a self, session: &'a SessionData) -> StorageFuture<'a, ()>;

    fn load_state(&self) -> StorageFuture<'_, Option<StoredState>>;

    fn save_state<'a>(&'a self, state: &'a StoredState) -> StorageFuture<'a, ()>;

    /// Inserts or replaces each peer whole, usernames and phone number included.
    fn update_peers<'a>(&'a self, peers: &'a [PeerRecord]) -> StorageFuture<'a, ()>;

    fn get_peer_by_id(&self, id: i64) -> StorageFuture<'_, Option<PeerRecord>>;

    /// `username` is lowercase, as [`PeerRecord::usernames`] are.
    fn get_peer_by_username<'a>(&'a self, username: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        let _ = username;
        Box::pin(async { Ok(None) })
    }

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
    pub access_hash: i64,
    /// Active usernames, lowercase.
    pub usernames: Vec<String>,
    pub phone_number: Option<String>,
    /// Unix time of the last change.
    pub updated: i64,
}

impl PeerRecord {
    pub fn new(id: i64, access_hash: i64) -> PeerRecord {
        PeerRecord { id, access_hash, ..PeerRecord::default() }
    }
}

/// The account the state belongs to, the server salt and where updates resume.
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StoredState {
    pub owner: i64,
    pub salt: Salt,
    pub update: Option<UpdateState>,
}

impl StoredState {
    pub fn new(owner: i64, salt: Salt, update: Option<UpdateState>) -> StoredState {
        StoredState { owner, salt, update }
    }
}
