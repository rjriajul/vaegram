use std::sync::Arc;
use std::time::SystemTime;

use super::DialogueKey;
use crate::storage::StorageFuture;

/// One conversation's saved state, and when it times out.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Record<S> {
    /// The state.
    pub state: S,
    /// When the state times out: from then on the conversation has no state. `None` keeps it until it's cleared.
    pub expires_at: Option<SystemTime>,
}

impl<S> Record<S> {
    /// `state`, kept until it's cleared.
    pub fn new(state: S) -> Self {
        Record { state, expires_at: None }
    }

    /// The same record, timing out at `expires_at`, or never for `None`.
    #[must_use]
    pub fn with_expiry(mut self, expires_at: Option<SystemTime>) -> Self {
        self.expires_at = expires_at;
        self
    }

    /// Whether the record has timed out at `now`.
    pub fn is_expired(&self, now: SystemTime) -> bool {
        self.expires_at.is_some_and(|at| at <= now)
    }
}

/// Where a [`DialogueRouter`](super::DialogueRouter) keeps its conversations' states, one [`Record`] per
/// [`DialogueKey`]. Separate from the session [`Storage`](crate::storage::Storage): logging out leaves it alone.
///
/// [`MemoryStorage`](super::MemoryStorage) keeps them in memory; `JsonFileStorage`, with the `dialogue-json`
/// feature, in a file. For another backend (Redis, SQL, ...) implement these three methods; the key's
/// [`Display`](std::fmt::Display) form can serve as the backend's key. A storage needn't drop expired records
/// itself: the dialogue checks [`Record::expires_at`] on every load and removes what has expired, though a backend
/// with its own expiry (such as Redis's) may use it too. A record the storage can no longer read, say after the
/// state type changed, is best returned as `None`, so the conversation starts over instead of failing for good.
pub trait DialogueStorage<S>: Send + Sync + 'static {
    /// The record under `key`, or `None`.
    fn get<'a>(&'a self, key: &'a DialogueKey) -> StorageFuture<'a, Option<Record<S>>>;

    /// Saves `record` under `key`, replacing what was there.
    fn set<'a>(&'a self, key: &'a DialogueKey, record: Record<S>) -> StorageFuture<'a, ()>;

    /// Removes the record under `key`; nothing there is fine.
    fn remove<'a>(&'a self, key: &'a DialogueKey) -> StorageFuture<'a, ()>;
}

impl<S, T: DialogueStorage<S> + ?Sized> DialogueStorage<S> for Arc<T> {
    fn get<'a>(&'a self, key: &'a DialogueKey) -> StorageFuture<'a, Option<Record<S>>> {
        (**self).get(key)
    }

    fn set<'a>(&'a self, key: &'a DialogueKey, record: Record<S>) -> StorageFuture<'a, ()> {
        (**self).set(key, record)
    }

    fn remove<'a>(&'a self, key: &'a DialogueKey) -> StorageFuture<'a, ()> {
        (**self).remove(key)
    }
}
