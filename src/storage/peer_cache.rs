use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{PeerRecord, Store};
use crate::raw::{base, types};
use crate::utils::{CHANNEL_OFFSET, get_channel_id};

/// The peers a client can reach without asking Telegram, in memory. Every change to a user or channel also goes
/// to the store, which writes it on its next flush.
pub(crate) struct PeerCache {
    peers: HashMap<i64, base::InputPeer>,
    // What the engine has, or has queued, for each peer: a peer seen again unchanged is never written again.
    known: HashMap<i64, PeerRecord>,
    store: Arc<Store>,
}

impl PeerCache {
    pub(crate) fn new(store: Arc<Store>) -> PeerCache {
        PeerCache { peers: HashMap::new(), known: HashMap::new(), store }
    }

    /// Writes later changes to `store` instead.
    #[cfg(test)]
    pub(crate) fn set_store(&mut self, store: Arc<Store>) {
        self.store = store;
    }

    pub(crate) fn get(&self, id: &i64) -> Option<&base::InputPeer> {
        self.peers.get(id)
    }

    #[cfg(test)]
    pub(crate) fn contains_key(&self, id: &i64) -> bool {
        self.peers.contains_key(id)
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.peers.len()
    }

    pub(crate) fn clear(&mut self) {
        self.peers.clear();
        self.known.clear();
    }

    pub(crate) fn insert(&mut self, id: i64, peer: base::InputPeer) -> Option<base::InputPeer> {
        if let Some(access_hash) = access_hash(&peer) {
            self.remember(id, access_hash, &[], None);
        }
        self.peers.insert(id, peer)
    }

    #[cfg(test)]
    pub(crate) fn extend(&mut self, peers: impl IntoIterator<Item = (i64, base::InputPeer)>) {
        for (id, peer) in peers {
            self.insert(id, peer);
        }
    }

    /// Remembers a user or channel as Telegram just described it. Only a change is written, and the comparison
    /// allocates nothing. `usernames` and `phone_number` are only compared and stored when the peer has any,
    /// so a bare access hash never erases what an earlier full description stored.
    pub(crate) fn remember(&mut self, id: i64, access_hash: i64, usernames: &[&str], phone_number: Option<&str>) {
        if let Some(peer) = input_peer(id, access_hash) {
            self.peers.insert(id, peer);
        }
        let full = !usernames.is_empty() || phone_number.is_some();
        let mut record = match self.known.get(&id) {
            Some(known) if known.access_hash == access_hash && (!full || same(known, usernames, phone_number)) => {
                return;
            }
            Some(known) if !full => known.clone(),
            _ => PeerRecord::new(id, access_hash),
        };
        record.access_hash = access_hash;
        if full {
            record.usernames = usernames.iter().map(|u| u.to_ascii_lowercase()).collect();
            record.phone_number = phone_number.map(str::to_owned);
        }
        record.updated = now();
        self.known.insert(id, record.clone());
        self.store.queue_peers([record]);
    }

    /// Fills the cache from the store without writing anything back.
    pub(crate) fn load(&mut self, records: impl IntoIterator<Item = PeerRecord>) {
        for record in records {
            if let Some(peer) = input_peer(record.id, record.access_hash) {
                self.peers.insert(record.id, peer);
            }
            self.known.insert(record.id, record);
        }
    }
}

fn same(known: &PeerRecord, usernames: &[&str], phone_number: Option<&str>) -> bool {
    known.phone_number.as_deref() == phone_number
        && known.usernames.len() == usernames.len()
        && known.usernames.iter().zip(usernames).all(|(k, u)| k.eq_ignore_ascii_case(u))
}

fn access_hash(peer: &base::InputPeer) -> Option<i64> {
    match peer {
        base::InputPeer::InputPeerUser(p) => Some(p.access_hash),
        base::InputPeer::InputPeerChannel(p) => Some(p.access_hash),
        _ => None,
    }
}

/// The input peer a stored record stands for: the id's sign tells users from channels. Basic groups need no hash
/// and aren't stored.
pub(crate) fn input_peer(id: i64, access_hash: i64) -> Option<base::InputPeer> {
    if id > 0 {
        Some(base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: id, access_hash })))
    } else if id <= -CHANNEL_OFFSET {
        let channel_id = get_channel_id(id);
        Some(base::InputPeer::InputPeerChannel(Box::new(types::InputPeerChannel { channel_id, access_hash })))
    } else {
        None
    }
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{PeerCache, input_peer};
    use crate::storage::{MemoryStorage, Storage, Store};

    fn cache() -> (PeerCache, Arc<Store>, Arc<MemoryStorage>) {
        let engine = Arc::new(MemoryStorage::new());
        let store = Arc::new(Store::new(engine.clone()));
        (PeerCache::new(store.clone()), store, engine)
    }

    #[tokio::test]
    async fn unchanged_peers_are_not_written_again() {
        let (mut cache, store, engine) = cache();
        cache.remember(100, 5, &["Alice"], Some("1555"));
        store.flush().await;
        let first = engine.get_peer_by_id(100).await.unwrap().unwrap();
        cache.remember(100, 5, &["alice"], Some("1555"));
        cache.insert(100, input_peer(100, 5).unwrap());
        store.flush().await;
        let second = engine.get_peer_by_id(100).await.unwrap().unwrap();
        assert_eq!(first.usernames, ["alice"]);
        assert_eq!(first, second, "nothing new was written");
        assert!(store.pending_is_empty());
    }

    #[tokio::test]
    async fn a_changed_access_hash_is_saved_without_run() {
        let (mut cache, store, engine) = cache();
        cache.remember(100, 5, &["alice"], None);
        cache.insert(100, input_peer(100, 6).unwrap());
        store.flush().await;
        let saved = engine.get_peer_by_id(100).await.unwrap().unwrap();
        assert_eq!((saved.access_hash, saved.usernames), (6, vec!["alice".to_owned()]), "the usernames stay");
    }

    #[tokio::test]
    async fn loaded_peers_are_reachable_and_not_written_back() {
        let (mut cache, store, _) = cache();
        let mut record = crate::storage::PeerRecord::new(-1_000_000_001_234, 9);
        record.usernames = vec!["news".into()];
        cache.load([record, crate::storage::PeerRecord::new(-42, 0)]);
        assert_eq!(cache.get(&-1_000_000_001_234), input_peer(-1_000_000_001_234, 9).as_ref());
        assert!(!cache.contains_key(&-42), "basic groups need no hash");
        cache.remember(-1_000_000_001_234, 9, &["news"], None);
        assert!(store.pending_is_empty());
    }
}
