use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError, Weak};

use tokio::sync::Mutex as Turn;

use super::DialogueKey;

/// How many keys the map holds before its first sweep of unused ones.
const FIRST_SWEEP: usize = 256;

/// One async lock per conversation, so its updates load, handle and save one at a time even when the client
/// handles updates concurrently. A lock lives while someone holds or waits for it; the map keeps only a weak
/// reference and sweeps dead ones once it has doubled in size since the last sweep.
#[derive(Default)]
pub(crate) struct KeyLocks(Mutex<Locks>);

#[derive(Default)]
struct Locks {
    map: HashMap<DialogueKey, Weak<Turn<()>>>,
    sweep_at: usize,
}

impl KeyLocks {
    /// The lock of `key`, the same one every caller gets while any of them still has it.
    pub(crate) fn get(&self, key: &DialogueKey) -> Arc<Turn<()>> {
        let mut guard = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let locks = &mut *guard;
        if let Some(lock) = locks.map.get(key).and_then(Weak::upgrade) {
            return lock;
        }
        if locks.map.len() >= locks.sweep_at.max(FIRST_SWEEP) {
            locks.map.retain(|_, lock| lock.strong_count() > 0);
            locks.sweep_at = locks.map.len() * 2;
        }
        let lock = Arc::new(Turn::new(()));
        locks.map.insert(key.clone(), Arc::downgrade(&lock));
        lock
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).map.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(user: i64) -> DialogueKey {
        DialogueKey::new("test", 1, user)
    }

    #[test]
    fn one_key_shares_a_lock_while_it_is_held() {
        let locks = KeyLocks::default();
        let held = locks.get(&key(1));
        assert!(Arc::ptr_eq(&held, &locks.get(&key(1))));
        assert!(!Arc::ptr_eq(&held, &locks.get(&key(2))));
    }

    #[test]
    fn unused_locks_are_swept() {
        let locks = KeyLocks::default();
        let kept = locks.get(&key(-1));
        for user in 0..FIRST_SWEEP as i64 * 4 {
            drop(locks.get(&key(user)));
        }
        // Only the held lock and the few made since the last sweep remain.
        assert!(locks.len() <= FIRST_SWEEP, "{} locks left", locks.len());
        assert!(Arc::ptr_eq(&kept, &locks.get(&key(-1))));
    }
}
