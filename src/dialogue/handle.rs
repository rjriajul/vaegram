use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime};

use super::{DialogueKey, DialogueStorage, Record};
use crate::errors::Result;

/// One conversation's state, as a dialogue step gets it: read it with [`state`](Self::state), move on with
/// [`set`](Self::set), end it with [`clear`](Self::clear). Each change is saved before the call returns. Clones
/// share the state.
///
/// Inside a step the conversation is locked: its next update waits until the step returns. A handle from
/// [`DialogueRouter::dialogue`](super::DialogueRouter::dialogue), outside any step, takes no lock.
pub struct Dialogue<S> {
    key: DialogueKey,
    storage: Arc<dyn DialogueStorage<S>>,
    timeout: Option<Duration>,
    state: Arc<Mutex<Option<S>>>,
}

impl<S> Clone for Dialogue<S> {
    fn clone(&self) -> Self {
        Dialogue {
            key: self.key.clone(),
            storage: Arc::clone(&self.storage),
            timeout: self.timeout,
            state: Arc::clone(&self.state),
        }
    }
}

impl<S: fmt::Debug> fmt::Debug for Dialogue<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Dialogue").field("key", &self.key).field("state", &*self.snapshot()).finish()
    }
}

impl<S> Dialogue<S> {
    fn snapshot(&self) -> MutexGuard<'_, Option<S>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The conversation's key.
    pub fn key(&self) -> &DialogueKey {
        &self.key
    }

    /// Whether the conversation has a state.
    pub fn is_active(&self) -> bool {
        self.snapshot().is_some()
    }
}

impl<S: Clone + Send + Sync + 'static> Dialogue<S> {
    /// Loads the state under `key`; one that timed out is removed and reads as none.
    pub(crate) async fn open(
        storage: Arc<dyn DialogueStorage<S>>,
        key: DialogueKey,
        timeout: Option<Duration>,
    ) -> Result<Self> {
        let state = match storage.get(&key).await? {
            Some(record) if record.is_expired(SystemTime::now()) => {
                storage.remove(&key).await?;
                None
            }
            record => record.map(|record| record.state),
        };
        Ok(Dialogue { key, storage, timeout, state: Arc::new(Mutex::new(state)) })
    }

    /// The current state, `None` when the conversation has none.
    pub fn state(&self) -> Option<S> {
        self.snapshot().clone()
    }

    /// Moves the conversation to `state`, which times out after the dialogue's
    /// [`timeout`](super::DialogueRouter::timeout) from now, if it has one.
    pub async fn set(&self, state: S) -> Result<()> {
        let expires_at = self.timeout.and_then(|timeout| SystemTime::now().checked_add(timeout));
        self.save(state, expires_at).await
    }

    /// Like [`set`](Self::set), but times out after `timeout`, whatever the dialogue's own timeout.
    pub async fn set_for(&self, state: S, timeout: Duration) -> Result<()> {
        self.save(state, SystemTime::now().checked_add(timeout)).await
    }

    async fn save(&self, state: S, expires_at: Option<SystemTime>) -> Result<()> {
        let record = Record::new(state.clone()).with_expiry(expires_at);
        self.storage.set(&self.key, record).await?;
        *self.snapshot() = Some(state);
        Ok(())
    }

    /// Changes the current state with `f` and saves it, as [`set`](Self::set) does. Returns `false`, doing
    /// nothing, when the conversation has no state.
    pub async fn update(&self, f: impl FnOnce(&mut S) + Send) -> Result<bool> {
        let Some(mut state) = self.state() else { return Ok(false) };
        f(&mut state);
        self.set(state).await?;
        Ok(true)
    }

    /// Ends the conversation: its state is removed, and the next update finds none.
    pub async fn clear(&self) -> Result<()> {
        self.storage.remove(&self.key).await?;
        *self.snapshot() = None;
        Ok(())
    }
}
