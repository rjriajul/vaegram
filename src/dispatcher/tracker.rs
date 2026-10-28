use std::cmp::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::Instant;

use super::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, types};
use crate::storage::state::UpdateState;

// https://core.telegram.org/api/updates#recovering-gaps
const GAP_TIMEOUT: Duration = Duration::from_millis(500);
// The same page tells clients to call getDifference after 15 minutes without updates, in case the server
// stopped pushing.
const IDLE_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const MAX_RETRIES: u32 = 5;

pub(super) fn channel_of(update: &base::Update) -> Option<i64> {
    let message = match update {
        base::Update::UpdateNewChannelMessage(u) => &u.message,
        base::Update::UpdateEditChannelMessage(u) => &u.message,
        base::Update::UpdateDeleteChannelMessages(u) => return Some(u.channel_id),
        base::Update::UpdateChannelWebPage(u) => return Some(u.channel_id),
        base::Update::UpdatePinnedChannelMessages(u) => return Some(u.channel_id),
        _ => return None,
    };
    let peer = match message {
        base::Message::Message(m) => &m.peer_id,
        base::Message::MessageService(m) => &m.peer_id,
        base::Message::MessageEmpty(m) => m.peer_id.as_ref()?,
    };
    match peer {
        base::Peer::PeerChannel(p) => Some(p.channel_id),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Key {
    Pts,
    Qts,
    Seq,
    Channel(i64),
}

pub(super) enum Item {
    Update(base::Update, Arc<Entities>),
    Batch(Vec<base::Update>, Arc<Entities>, i32, bool),
    Counter,
}

pub(super) struct Pending {
    pub(super) key: Key,
    pub(super) before: i32,
    pub(super) after: i32,
    pub(super) item: Item,
}

#[derive(Default)]
pub(super) struct Tracker {
    pub(super) state: UpdateState,
    pub(super) pending: Vec<(Instant, Pending)>,
    pub(super) retries: Vec<(Instant, Key)>,
    pub(super) failures: Vec<(Key, u32)>,
    pub(super) idle: Option<Instant>,
    pub(super) due: Vec<Key>,
    pub(super) ready: Vec<(base::Update, Arc<Entities>)>,
}

impl Tracker {
    pub(super) fn set_state(&mut self, s: &types::updates::State) {
        let state = &mut self.state;
        (state.pts, state.qts, state.date, state.seq) = (s.pts, s.qts, s.date, s.seq);
    }

    pub(super) fn counter(&mut self, key: Key) -> &mut i32 {
        match key {
            Key::Pts => &mut self.state.pts,
            Key::Qts => &mut self.state.qts,
            Key::Seq => &mut self.state.seq,
            Key::Channel(id) => self.state.channels.entry(id).or_insert(0),
        }
    }

    pub(super) fn mark(&mut self, key: Key) {
        let key = if let Key::Channel(_) = key { key } else { Key::Pts };
        // A scheduled retry (a FLOOD_WAIT or a back-off) already covers it; asking sooner only fails again.
        if !self.due.contains(&key) && !self.retries.iter().any(|(_, k)| *k == key) {
            self.due.push(key);
        }
    }

    pub(super) fn emit(&mut self, update: base::Update, entities: &Arc<Entities>) {
        self.ready.push((update, entities.clone()));
    }

    pub(super) fn deadline(&self) -> Option<Instant> {
        self.pending.iter().map(|(d, _)| *d).chain(self.retries.iter().map(|(d, _)| *d)).chain(self.idle).min()
    }

    pub(super) fn touch(&mut self, now: Instant) {
        self.idle = Some(now + IDLE_TIMEOUT);
    }

    pub(super) fn update(&mut self, update: base::Update, entities: &Arc<Entities>, quiet: bool) {
        if let base::Update::UpdateChannelTooLong(u) = &update {
            return self.mark(Key::Channel(u.channel_id));
        }
        let channel = channel_of(&update);
        let channel_message =
            matches!(update, base::Update::UpdateNewChannelMessage(_) | base::Update::UpdateEditChannelMessage(_));
        if channel.is_none() && channel_message {
            return;
        }
        let (key, before, after) = match (channel, update.pts(), update.qts()) {
            (Some(id), Some((pts, count)), _) => (Key::Channel(id), pts.wrapping_sub(count), pts),
            (Some(_), None, _) => return,
            (None, Some((pts, count)), _) => (Key::Pts, pts.wrapping_sub(count), pts),
            (None, None, Some(qts)) => (Key::Qts, qts.wrapping_sub(1), qts),
            (None, None, None) => {
                if !quiet {
                    self.emit(update, entities);
                }
                return;
            }
        };
        let item = if quiet { Item::Counter } else { Item::Update(update, entities.clone()) };
        self.offer(Pending { key, before, after, item });
    }

    pub(super) fn short(&mut self, pts: i32, count: i32, quiet: bool) {
        let before = pts.wrapping_sub(count);
        if quiet {
            self.offer(Pending { key: Key::Pts, before, after: pts, item: Item::Counter });
        } else if self.state.pts <= before {
            // Short updates lack the sender and chat; getDifference returns the full message.
            self.mark(Key::Pts);
        }
    }

    pub(super) fn batch(
        &mut self,
        updates: Vec<base::Update>,
        entities: Arc<Entities>,
        (date, seq_start, seq): (i32, i32, i32),
        quiet: bool,
    ) {
        if seq_start == 0 {
            for update in updates {
                self.update(update, &entities, quiet);
            }
        } else {
            let item = Item::Batch(updates, entities, date, quiet);
            self.offer(Pending { key: Key::Seq, before: seq_start.wrapping_sub(1), after: seq, item });
        }
    }

    pub(super) fn offer(&mut self, p: Pending) {
        let local = match p.key {
            Key::Channel(id) => *self.state.channels.entry(id).or_insert(p.before),
            key => *self.counter(key),
        };
        match local.cmp(&p.before) {
            Ordering::Greater => {}
            Ordering::Equal => {
                let key = p.key;
                self.apply(p);
                self.drain(key);
            }
            Ordering::Less => self.pending.push((Instant::now() + GAP_TIMEOUT, p)),
        }
    }

    pub(super) fn drain(&mut self, key: Key) {
        loop {
            let local = *self.counter(key);
            self.pending.retain(|(_, p)| p.key != key || p.before >= local);
            let Some(i) = self.pending.iter().position(|(_, p)| p.key == key && p.before == local) else { return };
            let (_, p) = self.pending.swap_remove(i);
            self.apply(p);
        }
    }

    pub(super) fn apply(&mut self, p: Pending) {
        let counter = self.counter(p.key);
        *counter = (*counter).max(p.after);
        match p.item {
            Item::Update(update, entities) => self.ready.push((update, entities)),
            Item::Batch(updates, entities, date, quiet) => {
                self.state.date = date;
                for update in updates {
                    self.update(update, &entities, quiet);
                }
            }
            Item::Counter => {}
        }
    }

    pub(super) fn expire(&mut self, now: Instant) {
        for i in 0..self.pending.len() {
            if self.pending[i].0 <= now {
                self.mark(self.pending[i].1.key);
            }
        }
        let mut i = 0;
        while i < self.retries.len() {
            if self.retries[i].0 <= now {
                let (_, key) = self.retries.swap_remove(i);
                self.mark(key);
            } else {
                i += 1;
            }
        }
        if self.idle.is_some_and(|idle| idle <= now) {
            self.mark(Key::Pts);
            self.touch(now);
        }
    }

    pub(super) fn recover(&mut self, result: &Result<()>, key: Key) {
        let due = |k: Key| match key {
            Key::Channel(_) => k == key,
            _ => !matches!(k, Key::Channel(_)),
        };
        let delay = match result {
            Ok(()) => None,
            // FLOOD_WAIT_X and friends: asking before X seconds pass only fails again.
            Err(Error::Rpc(e)) if e.code == 420 => Some(Duration::from_secs(e.value.unwrap_or(1).into())),
            Err(Error::Rpc(e)) if !(400..500).contains(&e.code) => self.backoff(key),
            Err(Error::Io(_) | Error::Transport(_) | Error::Disconnected | Error::Timeout | Error::BadMessage(_)) => {
                self.backoff(key)
            }
            // 4xx refusals and bad answers fail the same way next time, so deliver what we hold and give up on the gap.
            Err(_) => None,
        };
        self.retries.retain(|(_, k)| *k != key);
        let Some(delay) = delay else {
            self.failures.retain(|(k, _)| *k != key);
            return self.settle(due);
        };
        let deadline = Instant::now() + delay;
        for (d, _) in self.pending.iter_mut().filter(|(_, p)| due(p.key)) {
            *d = deadline;
        }
        // After updatesTooLong nothing may be pending, so the retry keeps its own deadline.
        self.retries.push((deadline, key));
    }

    fn backoff(&mut self, key: Key) -> Option<Duration> {
        let n = match self.failures.iter_mut().find(|(k, _)| *k == key) {
            Some((_, n)) => {
                *n += 1;
                *n
            }
            None => {
                self.failures.push((key, 1));
                1
            }
        };
        (n <= MAX_RETRIES).then(|| GAP_TIMEOUT * 2u32.pow(n - 1))
    }

    pub(super) fn settle(&mut self, due: impl Fn(Key) -> bool) {
        let (mut settled, rest): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.pending).into_iter().partition(|(_, p)| due(p.key));
        self.pending = rest;
        settled.sort_unstable_by_key(|(_, p)| (p.key, p.before));
        for (_, p) in settled {
            if *self.counter(p.key) < p.after {
                self.apply(p);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Arc, Duration, Error, GAP_TIMEOUT, IDLE_TIMEOUT, Instant, Key, Tracker, UpdateState, base, types};
    use crate::errors::RpcError;

    fn delete(pts: i32, pts_count: i32) -> base::Update {
        base::Update::UpdateDeleteMessages(Box::new(types::UpdateDeleteMessages {
            messages: Vec::new(),
            pts,
            pts_count,
        }))
    }

    fn stopped(qts: i32) -> base::Update {
        base::Update::UpdateBotStopped(Box::new(types::UpdateBotStopped { user_id: 1, date: 0, stopped: true, qts }))
    }

    fn channel(channel_id: i64, pts: i32) -> base::Update {
        base::Update::UpdateDeleteChannelMessages(Box::new(types::UpdateDeleteChannelMessages {
            channel_id,
            messages: Vec::new(),
            pts,
            pts_count: 1,
        }))
    }

    fn tracker(pts: i32, qts: i32, seq: i32) -> Tracker {
        Tracker { state: UpdateState { pts, qts, seq, ..UpdateState::default() }, ..Tracker::default() }
    }

    fn delivered(t: &mut Tracker) -> Vec<i32> {
        t.ready.drain(..).map(|(u, _)| u.pts().map_or_else(|| u.qts().unwrap_or(0), |(pts, _)| pts)).collect()
    }

    fn offline() -> Error {
        Error::Io(std::io::Error::other("offline"))
    }

    #[test]
    fn buffers_pts_gap_until_filled() {
        let (mut t, e) = (tracker(10, 0, 0), Arc::default());
        t.update(delete(12, 1), &e, false);
        assert!(delivered(&mut t).is_empty() && t.deadline().is_some());
        t.update(delete(11, 1), &e, false);
        assert_eq!((delivered(&mut t), t.state.pts, t.deadline()), (vec![11, 12], 12, None));
        t.update(delete(12, 1), &e, false);
        t.update(delete(14, 3), &e, false);
        assert_eq!((delivered(&mut t), t.pending.len()), (vec![], 0));
        t.update(delete(15, 3), &e, false);
        assert_eq!(delivered(&mut t), [15]);
    }

    #[test]
    fn expired_gap_requests_difference() {
        let (mut t, e) = (tracker(10, 0, 0), Arc::default());
        t.update(delete(12, 1), &e, false);
        t.expire(Instant::now());
        assert!(t.due.is_empty());
        t.expire(Instant::now() + GAP_TIMEOUT);
        assert_eq!(t.due, [Key::Pts]);
    }

    #[test]
    fn difference_covers_buffered_updates() {
        let (mut t, e) = (tracker(10, 0, 0), Arc::default());
        t.update(delete(12, 1), &e, false);
        t.update(delete(14, 1), &e, false);
        t.state.pts = 12;
        t.settle(|key| key == Key::Pts);
        assert_eq!((delivered(&mut t), t.state.pts, t.pending.len()), (vec![14], 14, 0));
    }

    #[test]
    fn empty_difference_moves_qts_forward() {
        let (mut t, e) = (tracker(0, 5, 0), Arc::default());
        t.update(stopped(7), &e, false);
        assert!(delivered(&mut t).is_empty());
        t.settle(|key| !matches!(key, Key::Channel(_)));
        assert_eq!((delivered(&mut t), t.state.qts), (vec![7], 7));
        t.update(stopped(8), &e, false);
        assert_eq!(delivered(&mut t), [8]);
    }

    #[test]
    fn seq_gap_buffers_whole_batch() {
        let (mut t, e) = (tracker(10, 0, 1), Arc::default());
        t.batch(vec![delete(12, 1)], Arc::clone(&e), (5, 3, 3), false);
        assert!(delivered(&mut t).is_empty());
        t.batch(vec![delete(11, 1)], Arc::clone(&e), (4, 2, 2), false);
        let s = &t.state;
        assert_eq!((s.seq, s.date, s.pts), (3, 5, 12));
        assert_eq!(delivered(&mut t), [11, 12]);
        t.batch(vec![stopped(1)], e, (6, 3, 3), false);
        assert!(delivered(&mut t).is_empty());
    }

    #[test]
    fn quiet_updates_move_counters_silently() {
        let (mut t, e) = (tracker(10, 0, 0), Arc::default());
        t.short(12, 1, true);
        t.update(delete(11, 1), &e, false);
        assert_eq!((delivered(&mut t), t.state.pts), (vec![11], 12));
        t.update(delete(13, 1), &e, true);
        assert_eq!((delivered(&mut t), t.state.pts), (vec![], 13));
    }

    #[test]
    fn failed_difference_keeps_the_gap() {
        let (mut t, e) = (tracker(10, 0, 0), Arc::default());
        t.update(delete(12, 1), &e, false);
        t.recover(&Err(offline()), Key::Pts);
        t.recover(&Err(Error::Timeout), Key::Pts);
        assert_eq!((delivered(&mut t), t.state.pts, t.pending.len()), (vec![], 10, 1));
        t.update(delete(11, 1), &e, false);
        assert_eq!(delivered(&mut t), [11, 12]);
    }

    #[test]
    fn refused_difference_delivers_held_updates() {
        let (mut t, e) = (tracker(10, 0, 0), Arc::default());
        t.update(delete(12, 1), &e, false);
        t.recover(&Err(Error::Rpc(RpcError::new(400, "CHANNEL_PRIVATE"))), Key::Pts);
        assert_eq!(delivered(&mut t), [12]);
        t.update(delete(14, 1), &e, false);
        t.recover(&Err(Error::Unexpected("bad difference")), Key::Pts);
        assert_eq!((delivered(&mut t), t.pending.len(), t.deadline()), (vec![14], 0, None));
    }

    #[test]
    fn silence_requests_difference() {
        let mut t = tracker(10, 0, 0);
        let start = Instant::now();
        t.touch(start);
        t.expire(start + IDLE_TIMEOUT - GAP_TIMEOUT);
        t.touch(start + GAP_TIMEOUT);
        t.expire(start + IDLE_TIMEOUT);
        assert!(t.due.is_empty());
        t.expire(start + GAP_TIMEOUT + IDLE_TIMEOUT);
        assert_eq!((t.due.clone(), t.deadline()), (vec![Key::Pts], Some(start + GAP_TIMEOUT + 2 * IDLE_TIMEOUT)));
    }

    #[test]
    fn flood_wait_retries_after_the_wait() {
        let (mut t, e) = (tracker(10, 0, 0), Arc::default());
        t.update(delete(12, 1), &e, false);
        t.recover(&Err(Error::Rpc(RpcError::new(420, "FLOOD_WAIT_30"))), Key::Pts);
        assert!(delivered(&mut t).is_empty());
        t.expire(Instant::now() + GAP_TIMEOUT);
        assert!(t.due.is_empty());
        t.expire(Instant::now() + Duration::from_secs(31));
        assert_eq!(t.due, [Key::Pts]);
    }

    #[test]
    fn new_gaps_wait_for_a_scheduled_retry() {
        let mut t = tracker(10, 0, 0);
        t.recover(&Err(Error::Rpc(RpcError::new(420, "FLOOD_WAIT_30"))), Key::Pts);
        t.mark(Key::Pts);
        t.mark(Key::Qts);
        assert!(t.due.is_empty());
        t.expire(Instant::now() + Duration::from_secs(31));
        assert_eq!(t.due, [Key::Pts]);
    }

    #[test]
    fn failed_catch_up_retries_with_nothing_held() {
        let mut t = tracker(10, 0, 0);
        t.recover(&Err(offline()), Key::Pts);
        t.recover(&Err(offline()), Key::Channel(1));
        assert!(t.deadline().is_some());
        t.expire(Instant::now() + GAP_TIMEOUT);
        assert_eq!((t.due.clone(), t.deadline()), (vec![Key::Pts, Key::Channel(1)], None));
        t.recover(&Err(offline()), Key::Pts);
        t.recover(&Ok(()), Key::Pts);
        assert!(t.deadline().is_none());
    }

    #[test]
    fn repeated_failures_back_off_then_give_up() {
        let (mut t, e) = (tracker(10, 0, 0), Arc::default());
        t.update(delete(12, 1), &e, false);
        for n in 0..5 {
            let start = Instant::now();
            t.recover(&Err(offline()), Key::Pts);
            assert!(t.deadline() >= Some(start + GAP_TIMEOUT * 2u32.pow(n)));
        }
        t.recover(&Err(offline()), Key::Pts);
        assert_eq!((delivered(&mut t), t.pending.len(), t.deadline()), (vec![12], 0, None));
    }

    #[test]
    fn quiet_channel_updates_move_channel_counters() {
        let (mut t, e) = (tracker(10, 0, 0), Arc::default());
        t.update(channel(5, 20), &e, true);
        t.update(channel(5, 21), &e, false);
        assert_eq!((delivered(&mut t), t.state.channels[&5], t.state.pts), (vec![21], 21, 10));
    }

    #[test]
    fn channel_message_without_peer_is_skipped() {
        let (mut t, e) = (tracker(10, 0, 0), Arc::default());
        let message = base::Message::MessageEmpty(Box::new(types::MessageEmpty { id: 1, peer_id: None }));
        let update = types::UpdateNewChannelMessage { message, pts: 11, pts_count: 1 };
        t.update(base::Update::UpdateNewChannelMessage(Box::new(update)), &e, false);
        assert_eq!((delivered(&mut t), t.state.pts, t.pending.len()), (vec![], 10, 0));
    }

    #[test]
    fn channel_gaps_are_per_channel() {
        let (mut t, e) = (tracker(0, 0, 0), Arc::default());
        t.update(channel(1, 5), &e, false);
        t.update(channel(2, 9), &e, false);
        assert_eq!(delivered(&mut t), [5, 9]);
        t.update(channel(1, 7), &e, false);
        t.update(channel(2, 10), &e, false);
        assert_eq!(delivered(&mut t), [10]);
        t.expire(Instant::now() + GAP_TIMEOUT);
        let too_long = types::UpdateChannelTooLong { channel_id: 1, pts: None };
        t.update(base::Update::UpdateChannelTooLong(Box::new(too_long)), &e, false);
        assert_eq!(t.due, [Key::Channel(1)]);
        t.state.channels.insert(1, 7);
        t.settle(|key| key == Key::Channel(1));
        assert_eq!((delivered(&mut t), t.pending.len()), (vec![], 0));
    }
}
