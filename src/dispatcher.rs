use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::Instant;

use crate::Client;
use crate::errors::{Error, Result};
use crate::handlers::{BoxFuture, Event, Handler};
use crate::raw::{base, functions, types};
use crate::session::{Incoming, UpdatesRx};
use crate::storage::state::UpdateState;
use crate::types::{CallbackQuery, InlineQuery, Message};
use crate::utils::get_channel_id;

const CHANNEL_DIFFERENCE_LIMIT: i32 = 100;
const SAVE_EVERY: Duration = Duration::from_secs(10);
const GAP_TIMEOUT: Duration = Duration::from_millis(500);
// Telegram asks for getDifference after 15 minutes without updates, in case the server stopped pushing.
const IDLE_TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[derive(Default)]
pub(crate) struct Entities {
    pub(crate) users: HashMap<i64, base::User>,
    pub(crate) chats: HashMap<i64, base::Chat>,
}

impl Entities {
    pub(crate) fn new(client: &Client, users: Vec<base::User>, chats: Vec<base::Chat>) -> Entities {
        let mut peers = client.0.peers.lock().unwrap();
        let mut entities = Entities::default();
        for user in users {
            if let base::User::User(u) = &user {
                if let (false, Some(access_hash)) = (u.min, u.access_hash) {
                    let peer = types::InputPeerUser { user_id: u.id, access_hash };
                    peers.insert(u.id, base::InputPeer::InputPeerUser(Box::new(peer)));
                }
                entities.users.insert(u.id, user);
            }
        }
        for chat in chats {
            let (id, peer) = match &chat {
                base::Chat::Chat(c) => (-c.id, None),
                base::Chat::ChatForbidden(c) => (-c.id, None),
                base::Chat::Channel(c) => {
                    let peer = match (c.min, c.access_hash) {
                        (false, Some(access_hash)) => Some(types::InputPeerChannel { channel_id: c.id, access_hash }),
                        _ => None,
                    };
                    (get_channel_id(c.id), peer)
                }
                base::Chat::ChannelForbidden(c) => (
                    get_channel_id(c.id),
                    Some(types::InputPeerChannel { channel_id: c.id, access_hash: c.access_hash }),
                ),
                _ => continue,
            };
            if let Some(peer) = peer {
                peers.insert(id, base::InputPeer::InputPeerChannel(Box::new(peer)));
            }
            entities.chats.insert(id, chat);
        }
        entities
    }
}

fn channel_of(update: &base::Update) -> Option<i64> {
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

fn new_message(message: base::Message) -> base::Update {
    base::Update::UpdateNewMessage(Box::new(types::UpdateNewMessage { message, pts: 0, pts_count: 0 }))
}

fn new_channel_message(message: base::Message) -> base::Update {
    base::Update::UpdateNewChannelMessage(Box::new(types::UpdateNewChannelMessage { message, pts: 0, pts_count: 0 }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Pts,
    Qts,
    Seq,
    Channel(i64),
}

enum Item {
    Update(base::Update, Arc<Entities>),
    Batch(Vec<base::Update>, Arc<Entities>, i32, bool),
    Counter,
}

struct Pending {
    key: Key,
    before: i32,
    after: i32,
    item: Item,
}

#[derive(Default)]
struct Tracker {
    state: UpdateState,
    pending: Vec<(Instant, Pending)>,
    retries: Vec<(Instant, Key)>,
    idle: Option<Instant>,
    due: Vec<Key>,
    ready: Vec<(base::Update, Arc<Entities>)>,
}

impl Tracker {
    fn set_state(&mut self, s: &types::updates::State) {
        let state = &mut self.state;
        (state.pts, state.qts, state.date, state.seq) = (s.pts, s.qts, s.date, s.seq);
    }

    fn counter(&mut self, key: Key) -> &mut i32 {
        match key {
            Key::Pts => &mut self.state.pts,
            Key::Qts => &mut self.state.qts,
            Key::Seq => &mut self.state.seq,
            Key::Channel(id) => self.state.channels.entry(id).or_insert(0),
        }
    }

    fn mark(&mut self, key: Key) {
        let key = if let Key::Channel(_) = key { key } else { Key::Pts };
        if !self.due.contains(&key) {
            self.due.push(key);
        }
    }

    fn emit(&mut self, update: base::Update, entities: &Arc<Entities>) {
        self.ready.push((update, entities.clone()));
    }

    fn deadline(&self) -> Option<Instant> {
        self.pending.iter().map(|(d, _)| *d).chain(self.retries.iter().map(|(d, _)| *d)).chain(self.idle).min()
    }

    fn touch(&mut self, now: Instant) {
        self.idle = Some(now + IDLE_TIMEOUT);
    }

    fn update(&mut self, update: base::Update, entities: &Arc<Entities>, quiet: bool) {
        if let base::Update::UpdateChannelTooLong(u) = &update {
            return self.mark(Key::Channel(u.channel_id));
        }
        let (key, before, after) = match (channel_of(&update), update.pts(), update.qts()) {
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

    fn short(&mut self, pts: i32, count: i32, quiet: bool) {
        let before = pts.wrapping_sub(count);
        if quiet {
            self.offer(Pending { key: Key::Pts, before, after: pts, item: Item::Counter });
        } else if self.state.pts <= before {
            // Short updates lack the sender and chat; getDifference returns the full message.
            self.mark(Key::Pts);
        }
    }

    fn batch(
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

    fn offer(&mut self, p: Pending) {
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

    fn drain(&mut self, key: Key) {
        loop {
            let local = *self.counter(key);
            self.pending.retain(|(_, p)| p.key != key || p.before >= local);
            let Some(i) = self.pending.iter().position(|(_, p)| p.key == key && p.before == local) else { return };
            let (_, p) = self.pending.swap_remove(i);
            self.apply(p);
        }
    }

    fn apply(&mut self, p: Pending) {
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

    fn expire(&mut self, now: Instant) {
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

    fn recover(&mut self, result: Result<()>, key: Key) {
        let due = |k: Key| match key {
            Key::Channel(_) => k == key,
            _ => !matches!(k, Key::Channel(_)),
        };
        let delay = match &result {
            Ok(()) => None,
            // FLOOD_WAIT_X and friends: asking before X seconds pass only fails again.
            Err(Error::Rpc(e)) if e.code == 420 => Some(Duration::from_secs(e.value.unwrap_or(1).into())),
            Err(Error::Rpc(e)) if !(400..500).contains(&e.code) => Some(GAP_TIMEOUT),
            Err(Error::Io(_) | Error::Transport(_) | Error::Disconnected | Error::Timeout | Error::BadMessage(_)) => {
                Some(GAP_TIMEOUT)
            }
            // 4xx refusals and bad answers fail the same way next time, so deliver what we hold and give up on the gap.
            Err(_) => None,
        };
        if let Err(e) = &result {
            eprintln!("vaegram: failed to fetch missed updates: {e}");
        }
        self.retries.retain(|(_, k)| *k != key);
        let Some(delay) = delay else { return self.settle(due) };
        let deadline = Instant::now() + delay;
        for (d, _) in self.pending.iter_mut().filter(|(_, p)| due(p.key)) {
            *d = deadline;
        }
        // After updatesTooLong nothing may be pending, so the retry keeps its own deadline.
        self.retries.push((deadline, key));
    }

    fn settle(&mut self, due: impl Fn(Key) -> bool) {
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

pub(crate) struct Dispatcher {
    client: Client,
    tracker: Tracker,
    saved_at: Instant,
    /// What the last save wrote, so an idle client doesn't rewrite the same file.
    saved: (UpdateState, usize),
}

impl Dispatcher {
    pub(crate) fn new(client: Client, state: &types::updates::State) -> Self {
        let mut tracker = Tracker::default();
        tracker.set_state(state);
        tracker.touch(Instant::now());
        Dispatcher { client, tracker, saved_at: Instant::now(), saved: Default::default() }
    }

    /// Starts from a saved state; the first pass of `run` fetches what was missed while offline.
    pub(crate) fn resume(client: Client, state: UpdateState) -> Self {
        let mut tracker = Tracker { state, due: vec![Key::Pts], ..Tracker::default() };
        tracker.touch(Instant::now());
        Dispatcher { client, tracker, saved_at: Instant::now(), saved: Default::default() }
    }

    pub(crate) async fn run(&mut self, updates: &mut UpdatesRx) {
        self.resolve().await;
        loop {
            for (update, entities) in std::mem::take(&mut self.tracker.ready) {
                self.dispatch(update, &entities);
            }
            // Saved every SAVE_EVERY when something moved, and on drop, so a crash replays at most that much.
            if self.saved_at.elapsed() >= SAVE_EVERY {
                self.saved_at = Instant::now();
                let now = (self.tracker.state.clone(), self.client.0.peers.lock().unwrap().len());
                if now != self.saved {
                    self.client.0.save_state_in_background(&now.0);
                    self.saved = now;
                }
            }
            let next = match self.tracker.deadline() {
                Some(deadline) => tokio::time::timeout_at(deadline, updates.recv()).await,
                None => Ok(updates.recv().await),
            };
            match next {
                Ok(Some(incoming)) => self.process(incoming),
                Ok(None) => return,
                Err(_) => self.tracker.expire(Instant::now()),
            }
            self.resolve().await;
        }
    }

    fn process(&mut self, incoming: Incoming) {
        let quiet = incoming.from_request;
        let t = &mut self.tracker;
        if !quiet {
            t.touch(Instant::now());
        }
        match incoming.updates {
            base::Updates::UpdatesTooLong => t.mark(Key::Pts),
            base::Updates::UpdateShortMessage(u) => t.short(u.pts, u.pts_count, quiet),
            base::Updates::UpdateShortChatMessage(u) => t.short(u.pts, u.pts_count, quiet),
            base::Updates::UpdateShortSentMessage(u) => t.short(u.pts, u.pts_count, true),
            base::Updates::UpdateShort(u) => t.update(u.update, &Arc::default(), quiet),
            base::Updates::Updates(u) => {
                let entities = Arc::new(Entities::new(&self.client, u.users, u.chats));
                t.batch(u.updates, entities, (u.date, u.seq, u.seq), quiet);
            }
            base::Updates::UpdatesCombined(u) => {
                let entities = Arc::new(Entities::new(&self.client, u.users, u.chats));
                t.batch(u.updates, entities, (u.date, u.seq_start, u.seq), quiet);
            }
        }
    }

    async fn resolve(&mut self) {
        while let Some(key) = self.tracker.due.pop() {
            let result = match key {
                Key::Channel(id) => self.channel_difference(id).await,
                _ => self.difference().await,
            };
            self.tracker.recover(result, key);
        }
    }

    async fn difference(&mut self) -> Result<()> {
        loop {
            let t = &mut self.tracker;
            let request = functions::updates::GetDifference {
                pts: t.state.pts,
                pts_limit: None,
                pts_total_limit: None,
                date: t.state.date,
                qts: t.state.qts,
                qts_limit: None,
            };
            let (messages, others, users, chats, state, done) = match self.client.invoke(&request).await? {
                base::updates::Difference::DifferenceEmpty(d) => {
                    (t.state.date, t.state.seq) = (d.date, d.seq);
                    return Ok(());
                }
                base::updates::Difference::DifferenceTooLong(d) => {
                    t.state.pts = d.pts;
                    return Ok(());
                }
                base::updates::Difference::Difference(d) => {
                    let d = *d;
                    (d.new_messages, d.other_updates, d.users, d.chats, d.state, true)
                }
                base::updates::Difference::DifferenceSlice(d) => {
                    let d = *d;
                    (d.new_messages, d.other_updates, d.users, d.chats, d.intermediate_state, false)
                }
            };
            let entities = Arc::new(Entities::new(&self.client, users, chats));
            for message in messages {
                t.emit(new_message(message), &entities);
            }
            for update in others {
                if matches!(update, base::Update::UpdateChannelTooLong(_)) || channel_of(&update).is_some() {
                    t.update(update, &entities, false);
                } else {
                    t.emit(update, &entities);
                }
            }
            let base::updates::State::State(state) = state;
            t.set_state(&state);
            if done {
                return Ok(());
            }
        }
    }

    async fn channel_difference(&mut self, channel_id: i64) -> Result<()> {
        let Some(&local) = self.tracker.state.channels.get(&channel_id) else { return Ok(()) };
        let mut pts = local;
        let channel = match self.client.resolve_peer(get_channel_id(channel_id)).await? {
            base::InputPeer::InputPeerChannel(p) => base::InputChannel::InputChannel(Box::new(types::InputChannel {
                channel_id,
                access_hash: p.access_hash,
            })),
            _ => return Ok(()),
        };
        loop {
            let request = functions::updates::GetChannelDifference {
                force: false,
                channel: channel.clone(),
                filter: base::ChannelMessagesFilter::ChannelMessagesFilterEmpty,
                pts,
                limit: CHANNEL_DIFFERENCE_LIMIT,
            };
            match self.client.invoke(&request).await? {
                base::updates::ChannelDifference::ChannelDifferenceEmpty(d) => {
                    pts = d.pts;
                    break;
                }
                base::updates::ChannelDifference::ChannelDifferenceTooLong(d) => {
                    let d = *d;
                    let entities = Arc::new(Entities::new(&self.client, d.users, d.chats));
                    for message in d.messages {
                        self.tracker.emit(new_channel_message(message), &entities);
                    }
                    if let base::Dialog::Dialog(dialog) = &d.dialog {
                        pts = dialog.pts.unwrap_or(pts);
                    }
                    break;
                }
                base::updates::ChannelDifference::ChannelDifference(d) => {
                    let d = *d;
                    let entities = Arc::new(Entities::new(&self.client, d.users, d.chats));
                    for message in d.new_messages {
                        self.tracker.emit(new_channel_message(message), &entities);
                    }
                    for update in d.other_updates {
                        self.tracker.emit(update, &entities);
                    }
                    pts = d.pts;
                    if d.r#final {
                        break;
                    }
                }
            }
        }
        self.tracker.state.channels.insert(channel_id, pts);
        Ok(())
    }

    fn dispatch(&self, update: base::Update, entities: &Entities) {
        let handlers = self.client.0.handlers.read().unwrap().clone();
        let mut event = self.event(&update, entities);
        for handler in &handlers {
            match handler {
                Handler::RawUpdate(callback) => spawn(callback(self.client.clone(), update.clone())),
                _ => {
                    if let Some(future) = Event::run(&mut event, handler) {
                        spawn(future);
                    }
                }
            }
        }
    }

    fn event(&self, update: &base::Update, entities: &Entities) -> Option<Event> {
        let client = &self.client;
        let message = |m: &base::Message| match m {
            base::Message::Message(m) => Some(Message::parse(client, m, entities)),
            _ => None,
        };
        match update {
            base::Update::UpdateNewMessage(u) => message(&u.message).map(Event::Message),
            base::Update::UpdateNewChannelMessage(u) => message(&u.message).map(Event::Message),
            base::Update::UpdateEditMessage(u) => message(&u.message).map(Event::EditedMessage),
            base::Update::UpdateEditChannelMessage(u) => message(&u.message).map(Event::EditedMessage),
            base::Update::UpdateBotCallbackQuery(u) => {
                Some(Event::CallbackQuery(CallbackQuery::parse(client, u, entities)))
            }
            base::Update::UpdateBotInlineQuery(u) => Some(Event::InlineQuery(InlineQuery::parse(client, u, entities))),
            _ => None,
        }
    }
}

impl Drop for Dispatcher {
    fn drop(&mut self) {
        if let Ok(mut state) = self.client.0.state.lock() {
            *state = Some(std::mem::take(&mut self.tracker.state));
        }
    }
}

fn spawn(future: BoxFuture) {
    tokio::spawn(async move {
        if let Err(e) = future.await {
            eprintln!("vaegram: handler failed: {e}");
        }
    });
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
        t.recover(Err(offline()), Key::Pts);
        t.recover(Err(Error::Timeout), Key::Pts);
        assert_eq!((delivered(&mut t), t.state.pts, t.pending.len()), (vec![], 10, 1));
        t.update(delete(11, 1), &e, false);
        assert_eq!(delivered(&mut t), [11, 12]);
    }

    #[test]
    fn refused_difference_delivers_held_updates() {
        let (mut t, e) = (tracker(10, 0, 0), Arc::default());
        t.update(delete(12, 1), &e, false);
        t.recover(Err(Error::Rpc(RpcError::new(400, "CHANNEL_PRIVATE"))), Key::Pts);
        assert_eq!(delivered(&mut t), [12]);
        t.update(delete(14, 1), &e, false);
        t.recover(Err(Error::Unexpected("bad difference")), Key::Pts);
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
        t.recover(Err(Error::Rpc(RpcError::new(420, "FLOOD_WAIT_30"))), Key::Pts);
        assert!(delivered(&mut t).is_empty());
        t.expire(Instant::now() + GAP_TIMEOUT);
        assert!(t.due.is_empty());
        t.expire(Instant::now() + Duration::from_secs(31));
        assert_eq!(t.due, [Key::Pts]);
    }

    #[test]
    fn failed_catch_up_retries_with_nothing_held() {
        let mut t = tracker(10, 0, 0);
        t.recover(Err(offline()), Key::Pts);
        t.recover(Err(offline()), Key::Channel(1));
        assert!(t.deadline().is_some());
        t.expire(Instant::now() + GAP_TIMEOUT);
        assert_eq!((t.due.clone(), t.deadline()), (vec![Key::Pts, Key::Channel(1)], None));
        t.recover(Err(offline()), Key::Pts);
        t.recover(Ok(()), Key::Pts);
        assert!(t.deadline().is_none());
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
