use std::cmp::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::Instant;

use super::Entities;
use crate::enums::SkipReason;
use crate::errors::{Error, Result};
use crate::raw::{base, types};
use crate::storage::UpdateState;

// https://core.telegram.org/api/updates#recovering-gaps
const GAP_TIMEOUT: Duration = Duration::from_millis(500);
// The same page tells clients to call getDifference after 15 minutes without updates, in case the server
// stopped pushing.
const IDLE_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const MAX_RETRIES: u32 = 5;
// Past this many held updates for one key the client is too far behind to fill the hole from what it holds, and the
// cap keeps a key that never fills from growing without bound. Held pts, qts and channel updates are dropped, since
// getDifference refetches them; held seq batches are released instead, since they also carry channel updates and
// updates that move no counter, which getDifference never returns.
const MAX_PENDING: usize = 1024;

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

#[derive(Debug, PartialEq)]
pub(super) struct Skipped {
    pub(super) key: Key,
    pub(super) from: i32,
    pub(super) to: i32,
    pub(super) reason: SkipReason,
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
    // Per key, the counter value the cap dropped held updates up to, kept until a fetch for the key settles: a fetch
    // given up on reports that range as skipped.
    pub(super) dropped: Vec<(Key, i32)>,
    pub(super) failures: Vec<(Key, u32)>,
    pub(super) idle: Option<Instant>,
    pub(super) due: Vec<Key>,
    pub(super) ready: Vec<(base::Update, Arc<Entities>)>,
    // updatePtsChanged came: the common box has to be read again with updates.getState.
    pub(super) refetch: bool,
}

impl Tracker {
    pub(super) fn set_state(&mut self, s: &types::updates::State) {
        let state = &mut self.state;
        (state.pts, state.qts, state.date, state.seq) = (s.pts, s.qts, s.date, s.seq);
    }

    /// The state updates.getState gave after updatePtsChanged. Updates held for a pts gap were numbered the old
    /// way, so they go; held qts and seq ones are checked against the fresh counters.
    pub(super) fn restate(&mut self, s: &types::updates::State) {
        self.set_state(s);
        self.pending.retain(|(_, p)| p.key != Key::Pts);
        self.dropped.retain(|(k, _)| *k != Key::Pts);
        self.drain(Key::Qts);
        self.drain(Key::Seq);
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
        // https://core.telegram.org/constructor/updatePtsChanged: the pts were renumbered. Raw handlers still get it.
        if matches!(update, base::Update::UpdatePtsChanged) {
            self.refetch = true;
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
            Ordering::Less => {
                if self.pending.iter().filter(|(_, held)| held.key == p.key).count() >= MAX_PENDING {
                    let key = p.key;
                    if key == Key::Seq {
                        // Give the seq hole up, but release every held batch in order through each update's own
                        // key: the seq counter moves past the hole, so the next batches apply whatever the
                        // difference leaves seq at, and getDifference refetches the pts and qts the hole held.
                        self.pending.push((Instant::now(), p));
                        self.settle(|k| k == Key::Seq, None);
                    } else {
                        // The hole is too deep to fill from the held updates: give the key up to getDifference,
                        // which refetches them from the stored state. The range dropped is kept, so a fetch given
                        // up on still reports it.
                        let mut to = p.after;
                        self.pending.retain(|(_, held)| {
                            if held.key == key {
                                to = to.max(held.after);
                            }
                            held.key != key
                        });
                        match self.dropped.iter_mut().find(|(k, _)| *k == key) {
                            Some((_, lost)) => *lost = (*lost).max(to),
                            None => self.dropped.push((key, to)),
                        }
                    }
                    self.mark(key);
                    return;
                }
                self.pending.push((Instant::now() + GAP_TIMEOUT, p));
            }
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
                // Batches can arrive out of date order; the state's date only moves forward, or getDifference
                // would be asked from before updates already applied.
                self.state.date = self.state.date.max(date);
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

    /// The pts ranges given up on, when the fetch for `key` won't be retried.
    pub(super) fn recover(&mut self, result: &Result<()>, key: Key) -> Vec<Skipped> {
        let due = |k: Key| match key {
            Key::Channel(_) => k == key,
            _ => !matches!(k, Key::Channel(_)),
        };
        let flood = result.as_ref().err().and_then(Error::flood_wait);
        let retry = match (result, flood) {
            (Ok(()), _) => Err(None),
            // FLOOD_WAIT_X and friends: asking before X seconds pass only fails again.
            (_, Some(wait)) => Ok(wait.max(Duration::from_secs(1))),
            (Err(Error::Rpc(e)), _) if !(400..500).contains(&e.code) => self.backoff(key),
            (
                Err(Error::Io(_) | Error::Transport(_) | Error::Disconnected | Error::Timeout | Error::BadMessage(_)),
                _,
            ) => self.backoff(key),
            // 4xx refusals and bad answers fail the same way next time, so deliver what we hold and give up on the gap.
            (Err(_), _) => Err(Some(SkipReason::Refused)),
        };
        self.retries.retain(|(_, k)| *k != key);
        let delay = match retry {
            Ok(delay) => delay,
            Err(reason) => {
                self.failures.retain(|(k, _)| *k != key);
                let mut skipped = self.settle(due, reason);
                // What the cap dropped past the last update delivered is lost too, unless the fetch read it.
                let dropped = self.dropped.iter().find(|(k, _)| *k == key).map(|(_, to)| *to);
                self.dropped.retain(|(k, _)| !due(*k));
                if let (Some(reason), Some(to), Key::Pts | Key::Channel(_)) = (reason, dropped, key) {
                    let at = self.counter(key);
                    if *at < to {
                        skipped.push(Skipped { key, from: *at, to, reason });
                        *at = to;
                    }
                }
                // A qts gap has no pts range to report.
                if let (Some(reason), true, Key::Pts | Key::Channel(_)) = (reason, skipped.is_empty(), key) {
                    let at = *self.counter(key);
                    skipped.push(Skipped { key, from: at, to: at, reason });
                }
                return skipped;
            }
        };
        let deadline = Instant::now() + delay;
        for (d, _) in self.pending.iter_mut().filter(|(_, p)| due(p.key)) {
            *d = deadline;
        }
        // After updatesTooLong nothing may be pending, so the retry keeps its own deadline.
        self.retries.push((deadline, key));
        Vec::new()
    }

    fn backoff(&mut self, key: Key) -> std::result::Result<Duration, Option<SkipReason>> {
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
        (n <= MAX_RETRIES).then(|| GAP_TIMEOUT * 2u32.pow(n - 1)).ok_or(Some(SkipReason::GaveUp))
    }

    /// Delivers what is held for the `due` keys; with a `reason`, the pts holes left before them are reported.
    pub(super) fn settle(&mut self, due: impl Fn(Key) -> bool, reason: Option<SkipReason>) -> Vec<Skipped> {
        let (mut settled, rest): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.pending).into_iter().partition(|(_, p)| due(p.key));
        self.pending = rest;
        settled.sort_unstable_by_key(|(_, p)| (p.key, p.before));
        let mut skipped = Vec::new();
        for (_, p) in settled {
            let local = *self.counter(p.key);
            if local >= p.after {
                continue;
            }
            if let (Some(reason), Key::Pts | Key::Channel(_)) = (reason, p.key) {
                if local < p.before {
                    skipped.push(Skipped { key: p.key, from: local, to: p.before, reason });
                }
            }
            self.apply(p);
        }
        skipped
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Arc, Duration, Entities, Error, GAP_TIMEOUT, IDLE_TIMEOUT, Instant, Key, MAX_PENDING, SkipReason, Skipped,
        Tracker, UpdateState, base, types,
    };
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
    fn a_key_over_the_pending_cap_falls_back_to_get_difference() {
        let (mut t, e) = (tracker(0, 0, 0), Arc::default());
        t.update(channel(5, 4), &e, false);
        t.update(channel(5, 6), &e, false);
        assert_eq!(delivered(&mut t), [4]);
        for pts in 2..=(MAX_PENDING as i32 + 1) {
            t.update(delete(pts, 1), &e, false);
        }
        assert_eq!((t.pending.len(), t.due.clone()), (MAX_PENDING + 1, vec![]));
        // The update past the cap hands the key to getDifference instead of growing the buffer; the other key keeps
        // its held updates, and neither counter moves.
        t.update(delete(MAX_PENDING as i32 + 2, 1), &e, false);
        assert_eq!((t.pending.len(), t.due.clone(), t.state.pts), (1, vec![Key::Pts], 0));

        // Per key: a channel over its own cap asks for that channel's difference, not the common box's.
        let (mut t, e) = (tracker(0, 0, 0), Arc::default());
        t.update(channel(5, 4), &e, false);
        for pts in 6..=(MAX_PENDING as i32 + 5) {
            t.update(channel(5, pts), &e, false);
        }
        assert_eq!((t.pending.len(), t.due.clone()), (MAX_PENDING, vec![]));
        t.update(channel(5, MAX_PENDING as i32 + 6), &e, false);
        assert_eq!((t.pending.len(), t.due.clone()), (0, vec![Key::Channel(5)]));
    }

    // Seq 2 never comes; each batch after it carries the next update of channel 5 and one that moves no counter.
    fn hold_seq_batches(t: &mut Tracker, e: &Arc<Entities>) -> i32 {
        for n in 1..=MAX_PENDING as i32 {
            t.batch(vec![channel(5, n), base::Update::UpdateConfig], Arc::clone(e), (n, n + 2, n + 2), false);
        }
        assert_eq!((t.ready.len(), t.pending.len(), t.due.clone()), (0, MAX_PENDING, vec![]));
        let last = MAX_PENDING as i32 + 1;
        t.batch(vec![channel(5, last)], Arc::clone(e), (last, last + 2, last + 2), false);
        last
    }

    #[test]
    fn seq_batches_over_the_cap_are_released_not_dropped() {
        let (mut t, e) = (tracker(0, 0, 1), Arc::default());
        let last = hold_seq_batches(&mut t, &e);
        // getDifference never returns channel updates or ones that move no counter, so every held batch goes out
        // in seq order through its updates' own keys before getDifference fetches what the hole held.
        let mut expected: Vec<i32> = (1..last).flat_map(|n| [n, 0]).collect();
        expected.push(last);
        let s = &t.state;
        assert_eq!((s.seq, s.date, s.channels[&5], s.pts), (last + 2, last, last, 0));
        assert_eq!((delivered(&mut t), t.pending.len(), t.due.clone()), (expected, 0, vec![Key::Pts]));
    }

    #[test]
    fn a_difference_that_leaves_seq_alone_does_not_refill_the_cap() {
        let (mut t, e) = (tracker(0, 0, 1), Arc::default());
        let last = hold_seq_batches(&mut t, &e);
        t.ready.clear();
        // updates.differenceTooLong with an unchanged pts sets no state, so seq stays where the release left it.
        assert_eq!(t.due.pop(), Some(Key::Pts));
        assert_eq!(t.recover(&Ok(()), Key::Pts), []);
        // The next batch follows on and goes straight out: nothing builds up towards the cap again.
        t.batch(vec![channel(5, last + 1)], e, (last + 1, last + 3, last + 3), false);
        assert_eq!((delivered(&mut t), t.pending.len(), t.due.clone()), (vec![last + 1], 0, vec![]));
        assert_eq!(t.state.seq, last + 3);
    }

    #[test]
    fn giving_up_after_the_cap_reports_what_it_dropped() {
        let (mut t, e) = (tracker(0, 0, 0), Arc::default());
        // A retry is already scheduled when pts 1 goes missing and the cap is reached, so marking the key is a no-op.
        t.recover(&Err(offline()), Key::Pts);
        let last = MAX_PENDING as i32 + 2;
        for pts in 2..=last {
            t.update(delete(pts, 1), &e, false);
        }
        assert_eq!((t.pending.len(), t.due.clone()), (0, vec![]));
        let skipped = t.recover(&Err(Error::Rpc(RpcError::parse(400, "CHANNEL_PRIVATE"))), Key::Pts);
        assert_eq!(skipped, [Skipped { key: Key::Pts, from: 0, to: last, reason: SkipReason::Refused }]);
        // The counter moves past what was given up on, so the next update goes straight out.
        t.update(delete(last + 1, 1), &e, false);
        assert_eq!((delivered(&mut t), t.state.pts), (vec![last + 1], last + 1));

        // A fetch that reads the hole forgets the dropped range: nothing is reported when a later one gives up.
        let (mut t, e) = (tracker(0, 0, 0), Arc::default());
        for pts in 2..=last {
            t.update(delete(pts, 1), &e, false);
        }
        t.state.pts = last;
        assert_eq!(t.recover(&Ok(()), Key::Pts), []);
        let refused = t.recover(&Err(Error::Rpc(RpcError::parse(400, "CHANNEL_PRIVATE"))), Key::Pts);
        assert_eq!(refused, [Skipped { key: Key::Pts, from: last, to: last, reason: SkipReason::Refused }]);
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
        t.settle(|key| key == Key::Pts, None);
        assert_eq!((delivered(&mut t), t.state.pts, t.pending.len()), (vec![14], 14, 0));
    }

    #[test]
    fn empty_difference_moves_qts_forward() {
        let (mut t, e) = (tracker(0, 5, 0), Arc::default());
        t.update(stopped(7), &e, false);
        assert!(delivered(&mut t).is_empty());
        t.settle(|key| !matches!(key, Key::Channel(_)), None);
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
    fn an_older_batch_does_not_move_the_date_back() {
        let (mut t, e) = (tracker(10, 0, 1), Arc::default());
        t.batch(vec![delete(11, 1)], Arc::clone(&e), (5, 2, 2), false);
        assert_eq!((t.state.seq, t.state.date), (2, 5));
        t.batch(vec![delete(12, 1)], e, (4, 3, 3), false);
        assert_eq!((delivered(&mut t), t.state.seq, t.state.date), (vec![11, 12], 3, 5));
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
        let skipped = t.recover(&Err(Error::Rpc(RpcError::parse(400, "CHANNEL_PRIVATE"))), Key::Pts);
        assert_eq!(skipped, [Skipped { key: Key::Pts, from: 10, to: 11, reason: SkipReason::Refused }]);
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
        t.recover(&Err(Error::Rpc(RpcError::parse(420, "FLOOD_WAIT_30"))), Key::Pts);
        assert!(delivered(&mut t).is_empty());
        t.expire(Instant::now() + GAP_TIMEOUT);
        assert!(t.due.is_empty());
        t.expire(Instant::now() + Duration::from_secs(31));
        assert_eq!(t.due, [Key::Pts]);
    }

    #[test]
    fn new_gaps_wait_for_a_scheduled_retry() {
        let mut t = tracker(10, 0, 0);
        t.recover(&Err(Error::Rpc(RpcError::parse(420, "FLOOD_WAIT_30"))), Key::Pts);
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
        let skipped = t.recover(&Err(offline()), Key::Pts);
        assert_eq!(skipped, [Skipped { key: Key::Pts, from: 10, to: 11, reason: SkipReason::GaveUp }]);
        assert_eq!((delivered(&mut t), t.pending.len(), t.deadline()), (vec![12], 0, None));
    }

    #[test]
    fn giving_up_with_nothing_held_reports_where_it_stopped() {
        let mut t = tracker(0, 0, 0);
        t.state.channels.insert(1, 7);
        let skipped = t.recover(&Err(Error::Rpc(RpcError::parse(400, "CHANNEL_PRIVATE"))), Key::Channel(1));
        assert_eq!(skipped, [Skipped { key: Key::Channel(1), from: 7, to: 7, reason: SkipReason::Refused }]);
        assert!(t.recover(&Ok(()), Key::Pts).is_empty());
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
        t.settle(|key| key == Key::Channel(1), None);
        assert_eq!((delivered(&mut t), t.pending.len()), (vec![], 0));
    }

    #[test]
    fn a_refused_qts_fetch_is_not_reported_as_a_pts_range() {
        let mut t = tracker(10, 5, 0);
        let refused = Err(Error::Rpc(crate::errors::RpcError::parse(400, "CHANNEL_PRIVATE")));
        assert_eq!(t.recover(&refused, Key::Qts), []);
    }
}
