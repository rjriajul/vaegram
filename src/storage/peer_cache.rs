use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{PeerRecord, Store};
use crate::raw::{base, types};
use crate::utils::{CHANNEL_OFFSET, get_channel_id};

/// How long a username learned from Telegram is trusted before it is looked up again: names change hands.
pub(crate) const USERNAME_TTL: Duration = Duration::from_secs(8 * 60 * 60);

/// Peers kept in memory. Past it the eighth seen longest ago goes; the store still has them, one lookup away.
const MAX_PEERS: usize = 100_000;

/// The peers a client can reach without asking Telegram, in memory, up to [`MAX_PEERS`]. Every change to a user
/// or channel also goes to the store, which writes it on its next flush.
pub(crate) struct PeerCache {
    peers: HashMap<i64, base::InputPeer>,
    // What the engine has, or has queued, for each peer: a peer seen again unchanged is never written again.
    known: HashMap<i64, PeerRecord>,
    // Each lowercase username and each number, with the `(updated, id)` of the newest peer that has it.
    usernames: HashMap<String, (i64, i64)>,
    phones: HashMap<String, (i64, i64)>,
    store: Arc<Store>,
    cap: usize,
}

impl PeerCache {
    pub(crate) fn new(store: Arc<Store>) -> PeerCache {
        PeerCache {
            peers: HashMap::new(),
            known: HashMap::new(),
            usernames: HashMap::new(),
            phones: HashMap::new(),
            store,
            cap: MAX_PEERS,
        }
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
        self.usernames.clear();
        self.phones.clear();
    }

    /// The peer with the lowercase `username`, and when that was last confirmed (unix seconds).
    pub(crate) fn by_username(&self, username: &str) -> Option<(i64, i64)> {
        self.usernames.get(username).map(|&(updated, id)| (id, updated))
    }

    pub(crate) fn by_phone_number(&self, phone_number: &str) -> Option<i64> {
        self.phones.get(phone_number).map(|&(_, id)| id)
    }

    pub(crate) fn insert(&mut self, id: i64, peer: base::InputPeer) -> Option<base::InputPeer> {
        if let Some(access_hash) = access_hash(&peer) {
            self.remember(id, access_hash, &[], None);
        }
        let old = self.peers.insert(id, peer);
        self.trim();
        old
    }

    #[cfg(test)]
    pub(crate) fn extend(&mut self, peers: impl IntoIterator<Item = (i64, base::InputPeer)>) {
        for (id, peer) in peers {
            self.insert(id, peer);
        }
    }

    /// Remembers a user or channel as Telegram just described it. Only a change is written, and the comparison
    /// allocates nothing. `usernames` and `phone_number` are only compared and stored when the peer has any,
    /// so a bare access hash never erases what an earlier full description stored. Seeing a peer's names again
    /// confirms them: past half of [`USERNAME_TTL`] the record is written again with a new time, so names in use
    /// stay fresh across restarts at about one write per peer every four hours.
    pub(crate) fn remember(&mut self, id: i64, access_hash: i64, usernames: &[&str], phone_number: Option<&str>) {
        if let Some(peer) = input_peer(id, access_hash) {
            self.peers.insert(id, peer);
        }
        let full = !usernames.is_empty() || phone_number.is_some();
        let now = now();
        let mut record = match self.known.get(&id) {
            Some(known) if known.access_hash == access_hash && (!full || same(known, usernames, phone_number)) => {
                if !full || now.saturating_sub(known.updated) <= USERNAME_TTL.as_secs() as i64 / 2 {
                    return;
                }
                known.clone()
            }
            Some(known) if !full => known.clone(),
            _ => PeerRecord::new(id, access_hash),
        };
        record.access_hash = access_hash;
        if full {
            record.usernames = usernames.iter().map(|u| u.to_ascii_lowercase()).collect();
            record.phone_number = phone_number.map(str::to_owned);
        }
        record.updated = now;
        self.keep(record.clone());
        self.store.queue_peers([record]);
    }

    /// Fills the cache from the store without writing anything back.
    pub(crate) fn load(&mut self, records: impl IntoIterator<Item = PeerRecord>) {
        for record in records {
            if let Some(peer) = input_peer(record.id, record.access_hash) {
                self.peers.insert(record.id, peer);
            }
            self.keep(record);
        }
    }

    fn keep(&mut self, record: PeerRecord) {
        self.forget(record.id);
        let claim = (record.updated, record.id);
        for name in &record.usernames {
            take(&mut self.usernames, name, claim);
        }
        if let Some(phone) = &record.phone_number {
            take(&mut self.phones, phone, claim);
        }
        self.known.insert(record.id, record);
        self.trim();
    }

    fn forget(&mut self, id: i64) {
        if let Some(old) = self.known.remove(&id) {
            for name in &old.usernames {
                release(&mut self.usernames, name, old.id);
            }
            if let Some(phone) = &old.phone_number {
                release(&mut self.phones, phone, old.id);
            }
        }
    }

    /// Past the cap, drops peers down to seven eighths of it: first the stored ones written longest ago, which a
    /// lookup gets back from the store, and only then the ones the store doesn't keep (basic groups, senders known
    /// only from a message), which come back when they're seen again.
    fn trim(&mut self) {
        if self.peers.len().max(self.known.len()) <= self.cap {
            return;
        }
        let keep = self.cap - self.cap / 8;
        let mut ages: Vec<(i64, i64)> = self.known.values().map(|r| (r.updated, r.id)).collect();
        ages.extend(self.peers.keys().filter(|id| !self.known.contains_key(id)).map(|&id| (i64::MAX, id)));
        let excess = ages.len().saturating_sub(keep);
        if excess == 0 {
            return;
        }
        ages.select_nth_unstable(excess - 1);
        for &(_, id) in &ages[..excess] {
            self.peers.remove(&id);
            self.forget(id);
        }
    }
}

// The newest claim keeps a name, so the owner doesn't depend on the order peers were seen in.
fn take(index: &mut HashMap<String, (i64, i64)>, key: &str, claim: (i64, i64)) {
    match index.get_mut(key) {
        Some(owner) if *owner >= claim => {}
        Some(owner) => *owner = claim,
        None => {
            index.insert(key.to_owned(), claim);
        }
    }
}

fn release(index: &mut HashMap<String, (i64, i64)>, key: &str, id: i64) {
    if index.get(key).is_some_and(|&(_, owner)| owner == id) {
        index.remove(key);
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
        let store = Arc::new(Store::new(engine.clone(), Arc::default()));
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
        (record.usernames, record.updated) = (vec!["news".into()], super::now());
        cache.load([record, crate::storage::PeerRecord::new(-42, 0)]);
        assert_eq!(cache.get(&-1_000_000_001_234), input_peer(-1_000_000_001_234, 9).as_ref());
        assert!(!cache.contains_key(&-42), "basic groups need no hash");
        cache.remember(-1_000_000_001_234, 9, &["news"], None);
        assert!(store.pending_is_empty());
    }

    #[tokio::test]
    async fn past_the_cap_the_cache_lets_go_of_peers_the_store_still_has() {
        let (mut cache, store, _) = cache();
        cache.cap = 8;
        for id in 1..=20 {
            let name = format!("user{id}");
            cache.remember(id, id * 3, &[&name], None);
        }
        let group = crate::raw::base::InputPeer::InputPeerChat(crate::raw::types::InputPeerChat { chat_id: 42 });
        cache.insert(-42, group);
        assert!(cache.len() <= 8 && cache.known.len() <= 8, "{} peers, {} records", cache.len(), cache.known.len());
        let gone = (1..=20).find(|id| cache.get(id).is_none()).expect("nothing was let go");
        assert!(cache.by_username(&format!("user{gone}")).is_none());
        // Not written yet: the store finds it in its queue.
        assert_eq!(store.get_peer(gone).await.unwrap().map(|p| p.access_hash), Some(gone * 3));
        let by_name = store.get_peer_by_username(format!("user{gone}")).await.unwrap();
        assert_eq!(by_name.map(|p| p.id), Some(gone));
        store.flush().await;
        assert!(store.pending_is_empty());
        assert_eq!(store.get_peer(gone).await.unwrap().map(|p| p.access_hash), Some(gone * 3));
    }
}
