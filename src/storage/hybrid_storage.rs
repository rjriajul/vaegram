use std::collections::HashMap;
use std::fmt;
use std::io;
use std::mem;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use tokio::sync::Notify;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use super::engine::{PeerRecord, Storage, StorageFuture, StorageReporter, StoredState};
use super::{MemoryStorage, Reports, SessionData};

const RETRY_MIN: Duration = Duration::from_secs(1);
const RETRY_MAX: Duration = Duration::from_secs(30);

/// Answers every read from a local engine and copies every write to a remote one in the background, so a slow or
/// unreachable remote never holds the client up.
///
/// The local side is a [`MemoryStorage`] unless [`local`](Self::local) says otherwise. Opening needs the remote. It
/// fills in what the local side lacks: the remote's login and state when the local side has none, and the
/// remote's newest peers unless the local side holds a newer version of them. The local login and state, when
/// there are any, are queued for the remote; local peers aren't sent at open, only when they are next written, so
/// a local peer is never replaced by an older one. After that, writes land locally first and queue for the remote,
/// merged by peer, and are retried from 1 up to every 30 seconds while the remote is down. When more peers queue up
/// than [`queue_size`](Self::queue_size) allows, the oldest peer writes are dropped (they stay local); the login
/// and the state are never dropped. Closing waits up to [`flush_timeout`](Self::flush_timeout) for the queue to
/// drain. Logging out drops every queued write and deletes on both sides; a remote delete that fails stays queued,
/// ahead of anything written since, and is retried like any other write.
pub struct HybridStorage {
    local: Box<dyn Storage>,
    remote: Arc<dyn Storage>,
    warm_peers: usize,
    flush_timeout: Duration,
    shared: Arc<Shared>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

struct Shared {
    pending: Mutex<Pending>,
    queue_size: usize,
    // Wakes the worker for new writes.
    wake: Notify,
    // Cuts the worker's wait between retries short, when closing.
    flush: Notify,
    // Fires when the queue drains.
    idle: Notify,
    // Held for every remote write, so a delete waits for the write in flight.
    remote: tokio::sync::Mutex<()>,
    reports: Reports,
}

#[derive(Default)]
struct Pending {
    queue: Queue,
    // The batch the worker is writing and the epoch it was taken in, put back if the write fails or is cut off.
    in_flight: Option<(Arc<Queue>, u64)>,
    // Bumped by a log-out, so a batch taken before it is never put back.
    epoch: u64,
}

#[derive(Clone, Default)]
struct Queue {
    // Delete everything on the remote before the writes below: a log-out that hasn't reached it yet.
    delete: bool,
    // Forget the remote's peers before writing the ones below.
    clear: bool,
    session: Option<SessionData>,
    state: Option<StoredState>,
    peers: HashMap<i64, PeerRecord>,
}

impl Queue {
    fn is_empty(&self) -> bool {
        !self.delete && !self.clear && self.session.is_none() && self.state.is_none() && self.peers.is_empty()
    }

    /// Puts a batch that didn't reach the remote back under the writes queued since, which are newer.
    fn put_back(&mut self, batch: Queue) {
        if !self.clear {
            for (id, peer) in batch.peers {
                self.peers.entry(id).or_insert(peer);
            }
            self.clear = batch.clear;
        }
        self.delete |= batch.delete;
        self.session = self.session.take().or(batch.session);
        self.state = self.state.take().or(batch.state);
    }

    /// Drops the oldest peer writes over `limit`, and returns how many.
    fn trim(&mut self, limit: usize) -> usize {
        let excess = self.peers.len().saturating_sub(limit);
        if excess == 0 {
            return 0;
        }
        let mut oldest: Vec<(i64, i64)> = self.peers.values().map(|p| (p.updated, p.id)).collect();
        oldest.sort_unstable();
        for (_, id) in &oldest[..excess] {
            self.peers.remove(id);
        }
        excess
    }
}

impl Shared {
    fn pending(&self) -> MutexGuard<'_, Pending> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Reports peer writes `trim` dropped; called with no lock held.
    fn dropped(&self, excess: usize) {
        if excess > 0 {
            let why = format!("the remote storage is behind; {excess} peer writes were dropped (they stay local)");
            self.reports.report(io::Error::other(why));
        }
    }

    fn enqueue(&self, f: impl FnOnce(&mut Queue)) {
        let mut pending = self.pending();
        f(&mut pending.queue);
        let excess = pending.queue.trim(self.queue_size);
        drop(pending);
        self.dropped(excess);
        self.wake.notify_one();
    }

    fn take(&self) -> Option<(Arc<Queue>, u64)> {
        let mut pending = self.pending();
        if pending.queue.is_empty() {
            return None;
        }
        let batch = Arc::new(mem::take(&mut pending.queue));
        pending.in_flight = Some((batch.clone(), pending.epoch));
        Some((batch, pending.epoch))
    }

    fn finish(&self, epoch: u64, failed: bool) {
        let mut pending = self.pending();
        let batch = pending.in_flight.take();
        let mut excess = 0;
        if let Some((batch, _)) = batch.filter(|_| failed && pending.epoch == epoch) {
            pending.queue.put_back(Arc::unwrap_or_clone(batch));
            excess = pending.queue.trim(self.queue_size);
        }
        if pending.queue.is_empty() {
            self.idle.notify_waiters();
        }
        drop(pending);
        self.dropped(excess);
    }

    /// Puts back a batch whose write was cut off, unless a log-out came after it was taken.
    fn recover(&self) {
        let mut pending = self.pending();
        let epoch = pending.epoch;
        let mut excess = 0;
        if let Some((batch, _)) = pending.in_flight.take().filter(|(_, taken)| *taken == epoch) {
            pending.queue.put_back(Arc::unwrap_or_clone(batch));
            excess = pending.queue.trim(self.queue_size);
        }
        drop(pending);
        self.dropped(excess);
    }

    fn drained(&self) -> bool {
        let pending = self.pending();
        pending.queue.is_empty() && pending.in_flight.is_none()
    }
}

async fn write(remote: &dyn Storage, batch: &Queue) -> io::Result<()> {
    if batch.delete {
        remote.delete().await?;
    }
    if batch.clear {
        remote.clear_peers().await?;
    }
    if let Some(session) = &batch.session {
        remote.save_session(session).await?;
    }
    if let Some(state) = &batch.state {
        remote.save_state(state).await?;
    }
    if !batch.peers.is_empty() {
        let peers: Vec<PeerRecord> = batch.peers.values().cloned().collect();
        remote.update_peers(&peers).await?;
    }
    Ok(())
}

async fn run(shared: Arc<Shared>, remote: Arc<dyn Storage>) {
    let mut delay = RETRY_MIN;
    loop {
        let guard = shared.remote.lock().await;
        let Some((batch, epoch)) = shared.take() else {
            drop(guard);
            shared.wake.notified().await;
            continue;
        };
        let written = write(&*remote, &batch).await;
        drop(batch);
        shared.finish(epoch, written.is_err());
        drop(guard);
        match written {
            Ok(()) => delay = RETRY_MIN,
            Err(e) => {
                let why = format!("couldn't write to the remote storage, retrying in {}s: {e}", delay.as_secs());
                shared.reports.report(io::Error::new(e.kind(), why));
                tokio::select! {
                    () = tokio::time::sleep(delay) => {},
                    () = shared.flush.notified() => {},
                }
                delay = (delay * 2).min(RETRY_MAX);
            }
        }
    }
}

impl HybridStorage {
    /// Copies every write to `remote` and answers reads from memory until [`local`](Self::local) says otherwise.
    pub fn new(remote: impl Storage) -> HybridStorage {
        HybridStorage {
            local: Box::new(MemoryStorage::new()),
            remote: Arc::new(remote),
            warm_peers: 4096,
            flush_timeout: Duration::from_secs(10),
            shared: Arc::new(Shared {
                pending: Mutex::new(Pending::default()),
                queue_size: 1024,
                wake: Notify::new(),
                flush: Notify::new(),
                idle: Notify::new(),
                remote: tokio::sync::Mutex::new(()),
                reports: Reports::default(),
            }),
            worker: Mutex::new(None),
        }
    }

    /// Answers reads from `engine` instead of from memory.
    pub fn local(mut self, engine: impl Storage) -> HybridStorage {
        self.local = Box::new(engine);
        self
    }

    /// How many peer writes may wait for the remote before the oldest are dropped; 1024 unless set. Set it before
    /// opening: once the storage has been opened it has no effect.
    pub fn queue_size(mut self, peers: usize) -> HybridStorage {
        if let Some(shared) = Arc::get_mut(&mut self.shared) {
            shared.queue_size = peers;
        }
        self
    }

    /// How many of the remote's newest peers opening copies to the local side; 4096 unless set. Set it before
    /// opening.
    pub fn warm_peers(mut self, peers: usize) -> HybridStorage {
        self.warm_peers = peers;
        self
    }

    /// How long closing waits for the queue to reach the remote; 10 seconds unless set.
    pub fn flush_timeout(mut self, timeout: Duration) -> HybridStorage {
        self.flush_timeout = timeout;
        self
    }

    fn queued(&self) -> usize {
        let pending = self.shared.pending();
        let q = &pending.queue;
        q.peers.len() + usize::from(q.session.is_some()) + usize::from(q.state.is_some()) + usize::from(q.clear)
    }

    /// Fills in what the local side lacks from the remote, and queues what the local side has for it.
    async fn warm(&self) -> io::Result<()> {
        match self.local.load_session().await? {
            Some(session) => self.shared.enqueue(|q| q.session = Some(session)),
            None => {
                if let Some(session) = self.remote.load_session().await? {
                    self.local.save_session(&session).await?;
                }
            }
        }
        match self.local.load_state().await? {
            Some(state) => self.shared.enqueue(|q| q.state = Some(state)),
            None => {
                if let Some(state) = self.remote.load_state().await? {
                    self.local.save_state(&state).await?;
                }
            }
        }
        let mut newer = Vec::new();
        for peer in self.remote.export_peers(Some(self.warm_peers)).await? {
            match self.local.get_peer_by_id(peer.id).await? {
                Some(known) if known.updated >= peer.updated => {}
                _ => newer.push(peer),
            }
        }
        if !newer.is_empty() {
            self.local.update_peers(&newer).await?;
        }
        Ok(())
    }

    async fn open(&self) -> io::Result<()> {
        self.local.open().await?;
        self.remote.open().await?;
        self.warm().await?;
        let mut worker = self.worker.lock().unwrap_or_else(PoisonError::into_inner);
        if worker.is_none() {
            *worker = Some(tokio::spawn(run(self.shared.clone(), self.remote.clone())));
        }
        Ok(())
    }

    async fn close(&self) -> io::Result<()> {
        let deadline = Instant::now() + self.flush_timeout;
        self.shared.flush.notify_one();
        let mut drained = true;
        loop {
            let idle = self.shared.idle.notified();
            tokio::pin!(idle);
            idle.as_mut().enable();
            if self.shared.drained() {
                break;
            }
            if tokio::time::timeout_at(deadline, idle).await.is_err() {
                drained = self.shared.drained();
                break;
            }
        }
        let worker = self.worker.lock().unwrap_or_else(PoisonError::into_inner).take();
        if let Some(worker) = worker {
            worker.abort();
            let _ = worker.await;
        }
        self.shared.recover();
        let local = self.local.close().await;
        let remote = self.remote.close().await;
        if !drained {
            let left = self.queued();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("{left} writes didn't reach the remote storage; they stay queued until it is opened again"),
            ));
        }
        local.and(remote)
    }

    async fn delete(&self) -> io::Result<()> {
        let epoch = {
            let mut pending = self.shared.pending();
            pending.queue = Queue { delete: true, ..Queue::default() };
            pending.epoch += 1;
            pending.epoch
        };
        let writing = self.shared.remote.lock().await;
        self.local.delete().await?;
        match self.remote.delete().await {
            Ok(()) => {
                let mut pending = self.shared.pending();
                if pending.epoch == epoch {
                    pending.queue.delete = false;
                }
                Ok(())
            }
            Err(e) => {
                drop(writing);
                self.shared.wake.notify_one();
                let why = format!("couldn't delete on the remote storage, it stays queued and is retried: {e}");
                Err(io::Error::new(e.kind(), why))
            }
        }
    }
}

impl Drop for HybridStorage {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.get_mut().unwrap_or_else(PoisonError::into_inner).take() {
            worker.abort();
        }
    }
}

impl fmt::Debug for HybridStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HybridStorage").field("queued", &self.queued()).finish_non_exhaustive()
    }
}

impl Storage for HybridStorage {
    // The wrapped engines see it too: the remote's own warnings (a Redis that evicts keys) come out of `open`.
    fn set_reporter(&self, reporter: StorageReporter) {
        self.shared.reports.set(reporter.clone());
        self.local.set_reporter(reporter.clone());
        self.remote.set_reporter(reporter);
    }

    fn open(&self) -> StorageFuture<'_, ()> {
        Box::pin(self.open())
    }

    fn close(&self) -> StorageFuture<'_, ()> {
        Box::pin(self.close())
    }

    fn delete(&self) -> StorageFuture<'_, ()> {
        Box::pin(self.delete())
    }

    fn load_session(&self) -> StorageFuture<'_, Option<SessionData>> {
        self.local.load_session()
    }

    fn save_session<'a>(&'a self, session: &'a SessionData) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.local.save_session(session).await?;
            self.shared.enqueue(|q| q.session = Some(session.clone()));
            Ok(())
        })
    }

    fn load_state(&self) -> StorageFuture<'_, Option<StoredState>> {
        self.local.load_state()
    }

    fn save_state<'a>(&'a self, state: &'a StoredState) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.local.save_state(state).await?;
            self.shared.enqueue(|q| q.state = Some(state.clone()));
            Ok(())
        })
    }

    fn update_peers<'a>(&'a self, peers: &'a [PeerRecord]) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            if peers.is_empty() {
                return Ok(());
            }
            self.local.update_peers(peers).await?;
            self.shared.enqueue(|q| q.peers.extend(peers.iter().map(|p| (p.id, p.clone()))));
            Ok(())
        })
    }

    fn get_peer_by_id(&self, id: i64) -> StorageFuture<'_, Option<PeerRecord>> {
        self.local.get_peer_by_id(id)
    }

    fn get_peer_by_username<'a>(&'a self, username: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        self.local.get_peer_by_username(username)
    }

    fn get_peer_by_phone_number<'a>(&'a self, phone_number: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
        self.local.get_peer_by_phone_number(phone_number)
    }

    fn export_peers(&self, limit: Option<usize>) -> StorageFuture<'_, Vec<PeerRecord>> {
        self.local.export_peers(limit)
    }

    fn clear_peers(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            self.local.clear_peers().await?;
            self.shared.enqueue(|q| {
                q.peers.clear();
                q.clear = true;
            });
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::Duration;

    use tokio::time::{Instant, sleep};

    use super::{HybridStorage, Queue};
    use crate::session::Salt;
    use crate::storage::{MemoryStorage, PeerRecord, SessionData, Storage, StorageFuture, StoredState, conformance};

    /// A remote that can go down and counts the calls it gets.
    #[derive(Default)]
    struct Remote {
        inner: MemoryStorage,
        down: AtomicBool,
        calls: AtomicUsize,
    }

    impl Remote {
        fn call<'a, T: Send + 'a>(&'a self, f: StorageFuture<'a, T>) -> StorageFuture<'a, T> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.down.load(Ordering::SeqCst) {
                return Box::pin(async { Err(io::Error::new(io::ErrorKind::ConnectionRefused, "down")) });
            }
            f
        }
    }

    impl Storage for Remote {
        fn open(&self) -> StorageFuture<'_, ()> {
            self.call(self.inner.open())
        }

        fn delete(&self) -> StorageFuture<'_, ()> {
            self.call(self.inner.delete())
        }

        fn load_session(&self) -> StorageFuture<'_, Option<SessionData>> {
            self.call(self.inner.load_session())
        }

        fn save_session<'a>(&'a self, session: &'a SessionData) -> StorageFuture<'a, ()> {
            self.call(self.inner.save_session(session))
        }

        fn load_state(&self) -> StorageFuture<'_, Option<StoredState>> {
            self.call(self.inner.load_state())
        }

        fn save_state<'a>(&'a self, state: &'a StoredState) -> StorageFuture<'a, ()> {
            self.call(self.inner.save_state(state))
        }

        fn update_peers<'a>(&'a self, peers: &'a [PeerRecord]) -> StorageFuture<'a, ()> {
            self.call(self.inner.update_peers(peers))
        }

        fn get_peer_by_id(&self, id: i64) -> StorageFuture<'_, Option<PeerRecord>> {
            self.call(self.inner.get_peer_by_id(id))
        }

        fn get_peer_by_username<'a>(&'a self, username: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
            self.call(self.inner.get_peer_by_username(username))
        }

        fn get_peer_by_phone_number<'a>(&'a self, phone: &'a str) -> StorageFuture<'a, Option<PeerRecord>> {
            self.call(self.inner.get_peer_by_phone_number(phone))
        }

        fn export_peers(&self, limit: Option<usize>) -> StorageFuture<'_, Vec<PeerRecord>> {
            self.call(self.inner.export_peers(limit))
        }

        fn clear_peers(&self) -> StorageFuture<'_, ()> {
            self.call(self.inner.clear_peers())
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

    fn state(owner: i64) -> StoredState {
        StoredState::new(owner, Salt { value: 1, time_offset: 0 }, None)
    }

    #[tokio::test]
    async fn conformance_on_memory_sides() {
        let remote = Arc::new(MemoryStorage::new());
        conformance::check(&HybridStorage::new(remote.clone())).await;
        assert!(remote.load_session().await.unwrap().is_none(), "delete reached the remote");
    }

    #[tokio::test]
    async fn writes_reach_the_remote_and_reads_never_touch_it() {
        let remote = Arc::new(Remote::default());
        let hybrid = HybridStorage::new(remote.clone());
        hybrid.open().await.unwrap();
        hybrid.save_session(&session(7)).await.unwrap();
        hybrid.update_peers(&[peer(1, 1)]).await.unwrap();
        let before = remote.calls.load(Ordering::SeqCst);
        remote.down.store(true, Ordering::SeqCst);
        assert_eq!(hybrid.load_session().await.unwrap().map(|s| s.user_id), Some(7));
        assert_eq!(hybrid.get_peer_by_username("user1").await.unwrap(), Some(peer(1, 1)));
        assert_eq!(hybrid.export_peers(None).await.unwrap(), [peer(1, 1)]);
        assert_eq!(remote.calls.load(Ordering::SeqCst), before, "reads stayed local");

        remote.down.store(false, Ordering::SeqCst);
        hybrid.close().await.unwrap();
        assert_eq!(remote.inner.load_session().await.unwrap().map(|s| s.user_id), Some(7));
        assert_eq!(remote.inner.export_peers(None).await.unwrap(), [peer(1, 1)]);
    }

    #[tokio::test(start_paused = true)]
    async fn remote_outage_keeps_reads_and_retries() {
        let remote = Arc::new(Remote::default());
        let hybrid = HybridStorage::new(remote.clone());
        hybrid.open().await.unwrap();
        remote.down.store(true, Ordering::SeqCst);
        let calls = remote.calls.load(Ordering::SeqCst);
        hybrid.save_state(&state(7)).await.unwrap();
        hybrid.update_peers(&[peer(1, 1)]).await.unwrap();
        assert_eq!(hybrid.load_state().await.unwrap(), Some(state(7)));

        // Tries at 0, 1, 3, 7 and 15 seconds: the waits double.
        sleep(Duration::from_millis(15_500)).await;
        assert_eq!(remote.calls.load(Ordering::SeqCst) - calls, 5);
        remote.down.store(false, Ordering::SeqCst);
        sleep(Duration::from_secs(16)).await;
        assert_eq!(remote.inner.load_state().await.unwrap(), Some(state(7)));
        assert_eq!(remote.inner.export_peers(None).await.unwrap(), [peer(1, 1)]);
        hybrid.close().await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn full_queue_drops_peers_first() {
        let remote = Arc::new(Remote::default());
        let hybrid = HybridStorage::new(remote.clone()).queue_size(2);
        hybrid.open().await.unwrap();
        remote.down.store(true, Ordering::SeqCst);
        hybrid.save_session(&session(7)).await.unwrap();
        hybrid.save_state(&state(7)).await.unwrap();
        hybrid.update_peers(&[peer(1, 1), peer(2, 2)]).await.unwrap();
        sleep(Duration::from_secs(2)).await;
        hybrid.update_peers(&[peer(3, 3)]).await.unwrap();
        assert_eq!(hybrid.export_peers(None).await.unwrap().len(), 3, "every peer stays local");

        remote.down.store(false, Ordering::SeqCst);
        hybrid.close().await.unwrap();
        assert_eq!(remote.inner.load_session().await.unwrap().map(|s| s.user_id), Some(7));
        assert_eq!(remote.inner.load_state().await.unwrap(), Some(state(7)));
        assert_eq!(remote.inner.export_peers(None).await.unwrap(), [peer(3, 3), peer(2, 2)]);
    }

    #[tokio::test(start_paused = true)]
    async fn dropped_peers_and_failed_writes_are_reported() {
        let remote = Arc::new(Remote::default());
        let hybrid = HybridStorage::new(remote.clone()).queue_size(2);
        let reports = conformance::collect_reports(&hybrid);
        hybrid.open().await.unwrap();
        remote.down.store(true, Ordering::SeqCst);
        hybrid.update_peers(&[peer(1, 1), peer(2, 2), peer(3, 3)]).await.unwrap();
        // The first retry waits a second, so only the first failed try has happened.
        sleep(Duration::from_millis(500)).await;
        let reports = reports.lock().unwrap().clone();
        remote.down.store(false, Ordering::SeqCst);
        hybrid.close().await.unwrap();
        let count = |what: &str| reports.iter().filter(|r| r.contains(what)).count();
        assert_eq!((count("peer writes were dropped"), count("couldn't write to the remote storage")), (1, 1));
        assert_eq!(reports.len(), 2, "{reports:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn close_waits_at_most_flush_timeout() {
        let remote = Arc::new(Remote::default());
        let hybrid = HybridStorage::new(remote.clone()).flush_timeout(Duration::from_secs(10));
        hybrid.open().await.unwrap();
        remote.down.store(true, Ordering::SeqCst);
        hybrid.save_state(&state(7)).await.unwrap();
        let start = Instant::now();
        let closed = hybrid.close().await;
        let waited = start.elapsed();
        let error = closed.unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(waited >= Duration::from_secs(10) && waited < Duration::from_secs(11), "{waited:?}");

        remote.down.store(false, Ordering::SeqCst);
        hybrid.open().await.unwrap();
        hybrid.close().await.unwrap();
        assert_eq!(remote.inner.load_state().await.unwrap(), Some(state(7)), "the queue outlived the close");
    }

    #[tokio::test]
    async fn warms_from_the_remote() {
        let remote = Arc::new(MemoryStorage::new());
        remote.save_session(&session(7)).await.unwrap();
        remote.save_state(&state(7)).await.unwrap();
        remote.update_peers(&[peer(1, 1), peer(2, 2)]).await.unwrap();
        let local = Arc::new(MemoryStorage::new());
        local.update_peers(&[peer(9, 9)]).await.unwrap();

        let hybrid = HybridStorage::new(remote.clone()).local(local.clone()).warm_peers(1);
        hybrid.open().await.unwrap();
        assert_eq!(local.load_session().await.unwrap().map(|s| s.user_id), Some(7));
        assert_eq!(local.load_state().await.unwrap(), Some(state(7)));
        assert_eq!(local.export_peers(None).await.unwrap(), [peer(9, 9), peer(2, 2)], "only the newest one");
        hybrid.close().await.unwrap();
    }

    #[tokio::test]
    async fn a_newer_local_side_catches_the_remote_up() {
        let remote = Arc::new(MemoryStorage::new());
        remote.save_session(&session(1)).await.unwrap();
        remote.update_peers(&[peer(5, 9)]).await.unwrap();
        let local = Arc::new(MemoryStorage::new());
        local.save_session(&session(2)).await.unwrap();
        local.update_peers(&[peer(5, 10)]).await.unwrap();

        let hybrid = HybridStorage::new(remote.clone()).local(local.clone());
        hybrid.open().await.unwrap();
        hybrid.close().await.unwrap();
        assert_eq!(local.get_peer_by_id(5).await.unwrap(), Some(peer(5, 10)), "an older remote peer is skipped");
        assert_eq!(remote.load_session().await.unwrap().map(|s| s.user_id), Some(2));
    }

    #[tokio::test(start_paused = true)]
    async fn a_failed_remote_delete_is_retried_before_later_writes() {
        let remote = Arc::new(Remote::default());
        remote.inner.save_session(&session(1)).await.unwrap();
        remote.inner.update_peers(&[peer(1, 1)]).await.unwrap();
        let hybrid = HybridStorage::new(remote.clone());
        hybrid.open().await.unwrap();

        remote.down.store(true, Ordering::SeqCst);
        let error = hybrid.delete().await.unwrap_err();
        assert!(error.to_string().contains("stays queued"), "{error}");
        assert!(hybrid.load_session().await.unwrap().is_none(), "the local side is gone at once");
        hybrid.save_session(&session(9)).await.unwrap();

        remote.down.store(false, Ordering::SeqCst);
        sleep(Duration::from_secs(2)).await;
        assert_eq!(remote.inner.load_session().await.unwrap().map(|s| s.user_id), Some(9), "the new login landed");
        assert!(remote.inner.export_peers(None).await.unwrap().is_empty(), "after the delete");
        hybrid.close().await.unwrap();
    }

    #[test]
    fn recover_never_restores_a_batch_from_before_a_log_out() {
        let hybrid = HybridStorage::new(MemoryStorage::new());
        let shared = &hybrid.shared;
        shared.enqueue(|q| q.session = Some(session(1)));
        let _cut_off = shared.take().unwrap();
        shared.recover();
        assert!(shared.pending().queue.session.is_some(), "a batch from this epoch comes back");

        let _cut_off = shared.take().unwrap();
        {
            let mut pending = shared.pending();
            pending.queue = Queue { delete: true, ..Queue::default() };
            pending.epoch += 1;
        }
        shared.recover();
        let pending = shared.pending();
        assert!(pending.queue.session.is_none() && pending.queue.delete && pending.in_flight.is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn log_out_stops_queued_writes() {
        let remote = Arc::new(Remote::default());
        let hybrid = HybridStorage::new(remote.clone());
        hybrid.open().await.unwrap();
        remote.down.store(true, Ordering::SeqCst);
        hybrid.save_session(&session(7)).await.unwrap();
        hybrid.update_peers(&[peer(1, 1)]).await.unwrap();
        sleep(Duration::from_millis(100)).await;

        remote.down.store(false, Ordering::SeqCst);
        hybrid.delete().await.unwrap();
        sleep(Duration::from_secs(60)).await;
        assert!(remote.inner.load_session().await.unwrap().is_none(), "the queued login never landed");
        assert!(remote.inner.export_peers(None).await.unwrap().is_empty());
        assert!(hybrid.load_session().await.unwrap().is_none());
        hybrid.close().await.unwrap();
    }

    #[cfg(feature = "redis")]
    #[tokio::test]
    #[ignore = "needs VAEGRAM_REDIS_URL, a Redis server it may write to"]
    async fn conformance_over_a_live_redis() {
        let Ok(url) = std::env::var("VAEGRAM_REDIS_URL") else {
            assert!(std::env::var_os("CI").is_none(), "set VAEGRAM_REDIS_URL");
            return;
        };
        let prefix = format!("vaegram:live-hybrid:{}", std::process::id());
        let remote = crate::storage::RedisStorage::new(&url, "bot").unwrap().prefix(prefix);
        conformance::check(&HybridStorage::new(remote)).await;
    }
}
