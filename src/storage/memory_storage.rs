use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::sync::{Mutex, MutexGuard, PoisonError};

use super::SessionData;
use super::engine::{PeerRecord, Storage, StorageFuture, StoredState};

// Every peer that claims a username or number, as (updated, id): the greatest one owns it, whatever order the
// records arrived in.
type Claims = HashMap<String, BTreeSet<(i64, i64)>>;

/// Everything an engine keeps, with the username and phone indexes. File engines replay into it.
#[derive(Default)]
pub(crate) struct Tables {
    pub(crate) session: Option<SessionData>,
    pub(crate) state: Option<StoredState>,
    peers: HashMap<i64, PeerRecord>,
    usernames: Claims,
    phones: Claims,
}

fn claim(claims: &mut Claims, key: &str, p: &PeerRecord) {
    claims.entry(key.to_owned()).or_default().insert((p.updated, p.id));
}

fn release(claims: &mut Claims, key: &str, p: &PeerRecord) {
    if let Some(owners) = claims.get_mut(key) {
        owners.remove(&(p.updated, p.id));
        if owners.is_empty() {
            claims.remove(key);
        }
    }
}

impl Tables {
    pub(crate) fn upsert(&mut self, record: PeerRecord) {
        if let Some(old) = self.peers.remove(&record.id) {
            for username in &old.usernames {
                release(&mut self.usernames, username, &old);
            }
            if let Some(phone) = &old.phone_number {
                release(&mut self.phones, phone, &old);
            }
        }
        for username in &record.usernames {
            claim(&mut self.usernames, username, &record);
        }
        if let Some(phone) = &record.phone_number {
            claim(&mut self.phones, phone, &record);
        }
        self.peers.insert(record.id, record);
    }

    pub(crate) fn clear_peers(&mut self) {
        self.peers.clear();
        self.usernames.clear();
        self.phones.clear();
    }

    /// Oldest first, so replaying them in this order rebuilds the same tables.
    pub(crate) fn peers_oldest_first(&self) -> Vec<&PeerRecord> {
        let mut oldest: Vec<&PeerRecord> = self.peers.values().collect();
        oldest.sort_unstable_by_key(|p| (p.updated, p.id));
        oldest
    }

    pub(crate) fn by_id(&self, id: i64) -> Option<PeerRecord> {
        self.peers.get(&id).cloned()
    }

    fn owner(&self, claims: &Claims, key: &str) -> Option<PeerRecord> {
        claims.get(key).and_then(|owners| owners.last()).and_then(|&(_, id)| self.by_id(id))
    }

    pub(crate) fn by_username(&self, username: &str) -> Option<PeerRecord> {
        self.owner(&self.usernames, username)
    }

    pub(crate) fn by_phone_number(&self, phone_number: &str) -> Option<PeerRecord> {
        self.owner(&self.phones, phone_number)
    }

    pub(crate) fn export(&self, limit: Option<usize>) -> Vec<PeerRecord> {
        let mut newest: Vec<&PeerRecord> = self.peers.values().collect();
        newest.sort_unstable_by(|a, b| b.updated.cmp(&a.updated).then(a.id.cmp(&b.id)));
        newest.into_iter().take(limit.unwrap_or(usize::MAX)).cloned().collect()
    }
}

/// Keeps everything in memory: nothing outlives the client unless the login is exported.
#[derive(Default)]
pub struct MemoryStorage(Mutex<Tables>);

impl MemoryStorage {
    /// An empty store: no login, state or peers.
    pub fn new() -> MemoryStorage {
        MemoryStorage::default()
    }

    fn tables(&self) -> MutexGuard<'_, Tables> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl fmt::Debug for MemoryStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryStorage").field("peers", &self.tables().peers.len()).finish_non_exhaustive()
    }
}

impl Storage for MemoryStorage {
    fn delete(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            *self.tables() = Tables::default();
            Ok(())
        })
    }

    fn load_session(&self) -> StorageFuture<'_, Option<SessionData>> {
        Box::pin(async move { Ok(self.tables().session.clone()) })
    }

    fn save_session<'a>(&'a self, session: &'a SessionData) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.tables().session = Some(session.clone());
            Ok(())
        })
    }

    fn load_state(&self) -> StorageFuture<'_, Option<StoredState>> {
        Box::pin(async move { Ok(self.tables().state.clone()) })
    }

    fn save_state<'a>(&'a self, state: &'a StoredState) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.tables().state = Some(state.clone());
            Ok(())
        })
    }

    fn update_peers<'a>(&'a self, peers: &'a [PeerRecord]) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let mut tables = self.tables();
            for peer in peers {
                tables.upsert(peer.clone());
            }
            Ok(())
        })
    }

    fn get_peer_by_id(&self, id: i64) -> StorageFuture<'_, Option<PeerRecord>> {
        Box::pin(async move { Ok(self.tables().by_id(id)) })
    }

    fn get_peer_by_username<'a>(&'a self, username: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        Box::pin(async move { Ok(self.tables().by_username(username)) })
    }

    fn get_peer_by_phone_number<'a>(&'a self, phone_number: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        Box::pin(async move { Ok(self.tables().by_phone_number(phone_number)) })
    }

    fn export_peers(&self, limit: Option<usize>) -> StorageFuture<'_, Vec<PeerRecord>> {
        Box::pin(async move { Ok(self.tables().export(limit)) })
    }

    fn clear_peers(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            self.tables().clear_peers();
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::MemoryStorage;

    #[tokio::test]
    async fn conformance() {
        super::super::conformance::check(&MemoryStorage::new()).await;
    }
}
