use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;

use super::{DialogueKey, DialogueStorage, Record};
use crate::storage::StorageFuture;

/// How many records a [`MemoryStorage`] holds before its first sweep of expired ones.
const FIRST_SWEEP: usize = 1024;

/// Keeps dialogue states in memory, for any `Clone` state type: nothing to serialize, and gone when the process
/// ends. Records that time out go on the next load of their key, or in a sweep once the storage has doubled in
/// size since the last one; [`vacuum`](Self::vacuum) sweeps at once.
pub struct MemoryStorage<S>(Mutex<Records<S>>);

struct Records<S> {
    map: HashMap<DialogueKey, Record<S>>,
    sweep_at: usize,
}

impl<S> MemoryStorage<S> {
    /// An empty storage.
    pub fn new() -> Self {
        MemoryStorage(Mutex::new(Records { map: HashMap::new(), sweep_at: FIRST_SWEEP }))
    }

    fn records(&self) -> MutexGuard<'_, Records<S>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// How many records it holds, expired ones not yet swept included.
    pub fn len(&self) -> usize {
        self.records().map.len()
    }

    /// Whether it holds no record.
    pub fn is_empty(&self) -> bool {
        self.records().map.is_empty()
    }

    /// Drops every expired record now; returns how many.
    pub fn vacuum(&self) -> usize {
        let mut records = self.records();
        let before = records.map.len();
        let now = SystemTime::now();
        records.map.retain(|_, record| !record.is_expired(now));
        before - records.map.len()
    }
}

impl<S> Default for MemoryStorage<S> {
    fn default() -> Self {
        MemoryStorage::new()
    }
}

impl<S> std::fmt::Debug for MemoryStorage<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryStorage").field("len", &self.len()).finish()
    }
}

impl<S: Clone + Send + Sync + 'static> DialogueStorage<S> for MemoryStorage<S> {
    fn get<'a>(&'a self, key: &'a DialogueKey) -> StorageFuture<'a, Option<Record<S>>> {
        let mut guard = self.records();
        let records = &mut *guard;
        let expired = records.map.get(key).is_some_and(|record| record.is_expired(SystemTime::now()));
        let found = if expired {
            records.map.remove(key);
            None
        } else {
            records.map.get(key).cloned()
        };
        drop(guard);
        Box::pin(std::future::ready(Ok(found)))
    }

    fn set<'a>(&'a self, key: &'a DialogueKey, record: Record<S>) -> StorageFuture<'a, ()> {
        let mut guard = self.records();
        let records = &mut *guard;
        records.map.insert(key.clone(), record);
        if records.map.len() >= records.sweep_at {
            let now = SystemTime::now();
            records.map.retain(|_, record| !record.is_expired(now));
            records.sweep_at = (records.map.len() * 2).max(FIRST_SWEEP);
        }
        drop(guard);
        Box::pin(std::future::ready(Ok(())))
    }

    fn remove<'a>(&'a self, key: &'a DialogueKey) -> StorageFuture<'a, ()> {
        self.records().map.remove(key);
        Box::pin(std::future::ready(Ok(())))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn key(user: i64) -> DialogueKey {
        DialogueKey::new("test", 1, user)
    }

    #[tokio::test]
    async fn round_trips_and_removes() {
        let storage = MemoryStorage::<String>::new();
        assert_eq!(storage.get(&key(1)).await.unwrap(), None);
        storage.set(&key(1), Record::new("name".to_owned())).await.unwrap();
        storage.set(&key(2), Record::new("age".to_owned())).await.unwrap();
        assert_eq!(storage.get(&key(1)).await.unwrap(), Some(Record::new("name".to_owned())));
        assert_eq!(storage.get(&key(2)).await.unwrap().map(|r| r.state), Some("age".to_owned()));
        storage.remove(&key(1)).await.unwrap();
        assert_eq!(storage.get(&key(1)).await.unwrap(), None);
        assert_eq!(storage.len(), 1);
    }

    #[tokio::test]
    async fn an_expired_record_is_gone_on_load_and_on_vacuum() {
        let storage = MemoryStorage::<i32>::new();
        let past = SystemTime::now() - Duration::from_secs(1);
        let future = SystemTime::now() + Duration::from_secs(3600);
        storage.set(&key(1), Record::new(1).with_expiry(Some(past))).await.unwrap();
        storage.set(&key(2), Record::new(2).with_expiry(Some(past))).await.unwrap();
        storage.set(&key(3), Record::new(3).with_expiry(Some(future))).await.unwrap();
        assert_eq!(storage.get(&key(1)).await.unwrap(), None);
        assert_eq!(storage.len(), 2);
        assert_eq!(storage.vacuum(), 1);
        assert_eq!(storage.get(&key(3)).await.unwrap().map(|r| r.state), Some(3));
    }

    #[tokio::test]
    async fn growth_sweeps_expired_records() {
        let storage = MemoryStorage::<i32>::new();
        let past = SystemTime::now() - Duration::from_secs(1);
        for user in 0..FIRST_SWEEP as i64 {
            storage.set(&key(user), Record::new(0).with_expiry(Some(past))).await.unwrap();
        }
        // The record that reached the threshold swept every expired one, itself included.
        assert_eq!(storage.len(), 0);
    }
}
