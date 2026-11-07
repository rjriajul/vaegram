pub(crate) mod cache;
mod difference;
mod entities;
pub(crate) mod listeners;
pub(crate) mod router;
mod tasks;
mod tracker;

use std::sync::atomic::Ordering;
use std::sync::{Arc, PoisonError};
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

pub(crate) use entities::Entities;
pub(crate) use router::{Router, Routes};
use tasks::Tasks;
use tracker::{Key, Tracker};

use crate::Client;
use crate::enums::{ClientEvent, SkipReason};
use crate::errors::Result;
use crate::raw::{base, types};
use crate::session::{Incoming, UpdatesRx};
use crate::storage::UpdateState;
use crate::utils::{CHANNEL_OFFSET, channel_id_of, get_channel_id};

const SAVE_EVERY: Duration = Duration::from_secs(10);

/// A `recover_gaps` call for the running dispatcher to carry out.
pub(crate) struct Recover {
    /// `None` for every box; otherwise 0 for the common box and `-100…` ids for channels.
    pub(crate) chats: Option<Vec<i64>>,
    /// Messages, then other updates, that came from the recovery.
    pub(crate) reply: oneshot::Sender<(usize, usize)>,
}

pub(crate) struct Dispatcher {
    client: Client,
    tracker: Tracker,
    saved_at: Instant,
    saved: UpdateState,
    router: Arc<Router>,
    generation: u64,
    account: u64,
    tasks: Tasks,
    // Messages and other updates the differences have brought in, counted by `difference.rs`.
    recovered: (usize, usize),
    // Set by `from_now` until the first difference is read whole: what it brings dated before this, Telegram held
    // from before the run, and it is dropped unseen.
    held_before: Option<i32>,
}

impl Dispatcher {
    pub(crate) fn new(client: Client, state: &types::updates::State) -> Self {
        let mut tracker = Tracker::default();
        tracker.set_state(state);
        Dispatcher::with(client, tracker)
    }

    pub(crate) fn resume(client: Client, state: UpdateState) -> Self {
        Dispatcher::with(client, Tracker { state, due: vec![Key::Pts], ..Tracker::default() })
    }

    /// Starts at `state` without catch-up. A bot's updates.getState names the last update Telegram knows it got, and
    /// the ones after it come again, even those an earlier run handled, so `run` reads them first: those dated
    /// before `since` are dropped, and those dated later, which came in since the run began, reach the handlers.
    /// Updates that carry no date, such as deletions and poll votes, can't be told apart and are dropped too.
    pub(crate) fn from_now(client: Client, state: &types::updates::State, since: i32) -> Self {
        let mut tracker = Tracker { due: vec![Key::Pts], ..Tracker::default() };
        tracker.set_state(state);
        let mut dispatcher = Dispatcher::with(client, tracker);
        dispatcher.held_before = Some(since);
        dispatcher
    }

    fn with(client: Client, mut tracker: Tracker) -> Self {
        tracker.touch(Instant::now());
        let generation = client.0.handlers.generation();
        let account = client.0.account.load(Ordering::Relaxed);
        let router = client.0.handlers.current();
        let mut tasks = Tasks::new(client.0.ordered, client.0.handler_permits.clone());
        tasks.on_error = router.on_error.clone();
        tasks.events = client.0.events.clone();
        let saved = Default::default();
        Dispatcher {
            client,
            tracker,
            saved_at: Instant::now(),
            saved,
            router,
            generation,
            account,
            tasks,
            recovered: (0, 0),
            held_before: None,
        }
    }

    fn same_account(&self) -> bool {
        self.client.0.account.load(Ordering::Relaxed) == self.account
    }

    /// `true` when `Client::stop` ended it, after the running handlers finished; `false` when the connection did.
    pub(crate) async fn run(&mut self, updates: &mut UpdatesRx) -> bool {
        let mut stop = self.client.0.stop.subscribe();
        self.client.0.handlers.listeners.open();
        let (requests, mut recoveries) = mpsc::unbounded_channel();
        *self.client.0.recover.lock().unwrap_or_else(PoisonError::into_inner) = Some(requests);
        self.resolve().await;
        loop {
            self.dispatch_ready();
            // Saved every SAVE_EVERY when it moved, and on drop, so a crash replays at most that much. Peers are
            // written by the store on its own timer.
            if self.saved_at.elapsed() >= SAVE_EVERY {
                self.saved_at = Instant::now();
                if self.tracker.state != self.saved && self.same_account() {
                    self.client.0.save_state_in_background(&self.tracker.state);
                    self.saved = self.tracker.state.clone();
                }
            }
            let deadline = self.tracker.deadline();
            let next = async {
                match deadline {
                    Some(deadline) => tokio::time::timeout_at(deadline, updates.recv()).await,
                    None => Ok(updates.recv().await),
                }
            };
            // With a handler cap and the waiting room full, updates wait in the update queue instead.
            let full = self.tasks.full();
            let next = tokio::select! {
                biased;
                // The watch guard `wait_for` returns isn't Send, so it is dropped before awaiting the handlers.
                _ = async { drop(stop.wait_for(|stopped| *stopped).await) } => {
                    // A handler waiting on `recover_gaps` would hold up `finish` for ever: nothing answers it any
                    // more, so the callers still queued get `Disconnected` and later ones are refused.
                    *self.client.0.recover.lock().unwrap_or_else(PoisonError::into_inner) = None;
                    recoveries.close();
                    while recoveries.try_recv().is_ok() {}
                    // Before waiting: a handler parked in `listen` would otherwise hold `stop` up to its timeout.
                    self.client.0.handlers.listeners.close();
                    self.tasks.finish().await;
                    if self.same_account() {
                        self.client.0.save_state(&self.tracker.state).await;
                    }
                    return true;
                }
                next = next, if !full => next,
                () = self.tasks.one_done(), if full => continue,
                // After the updates, and one request a turn, so a tight `recover_gaps` loop can't starve them.
                Some(request) = recoveries.recv() => {
                    self.recover(request).await;
                    continue;
                }
                () = self.client.0.handlers.listeners.returned().notified() => {
                    self.redeliver();
                    continue;
                }
            };
            match next {
                Ok(Some(incoming)) => self.process(incoming),
                Ok(None) => {
                    // The connection is gone for good: save here, since the next save is on drop, which a crash skips.
                    if self.same_account() {
                        self.client.0.save_state(&self.tracker.state).await;
                    }
                    return false;
                }
                Err(_) => self.tracker.expire(Instant::now()),
            }
            self.resolve().await;
        }
    }

    /// Marks what was asked for as due and resolves it now, so the counts include whatever else was already due; the
    /// updates it finds reach the handlers page by page, the last page's on the next turn.
    async fn recover(&mut self, request: Recover) {
        let t = &mut self.tracker;
        match request.chats {
            None => {
                t.mark(Key::Pts);
                let channels: Vec<i64> = t.state.channels.keys().copied().collect();
                channels.into_iter().for_each(|id| t.mark(Key::Channel(id)));
            }
            Some(chats) => {
                for chat in chats {
                    if chat == 0 {
                        t.mark(Key::Pts);
                    } else if chat <= -CHANNEL_OFFSET && t.state.channels.contains_key(&channel_id_of(chat)) {
                        // Only `-100…` ids are channels; a user's or group's id names no box.
                        t.mark(Key::Channel(channel_id_of(chat)));
                    }
                }
            }
        }
        let before = self.recovered;
        self.resolve().await;
        // The caller may have stopped waiting.
        let _ = request.reply.send((self.recovered.0 - before.0, self.recovered.1 - before.1));
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
        // Before any getDifference, which would otherwise start from the old numbering.
        if std::mem::take(&mut self.tracker.refetch) {
            let result = self.refetch_state().await;
            // A failure is reported like a failed difference and asked again on the next turn.
            self.tracker.refetch = result.is_err();
            self.report(result);
        }
        while let Some(key) = self.tracker.due.pop() {
            // Nothing waits in `ready` across an await: a dropped `run` would hand back a state past updates no
            // handler saw.
            self.dispatch_ready();
            let result = match key {
                Key::Channel(id) => self.channel_difference(id).await,
                _ => {
                    let read = self.difference().await;
                    // Only the first difference read whole holds what came before the run.
                    if read.is_ok() {
                        self.held_before = None;
                    }
                    read
                }
            };
            let skipped = self.tracker.recover(&result, key);
            self.report(result);
            for s in skipped {
                self.skipped(s.key, s.from, s.to, s.reason);
            }
        }
    }

    fn report(&self, result: Result<()>) {
        if let Err(error) = result {
            self.client.0.events.emit(ClientEvent::DifferenceFailed { error: Arc::new(error) });
        }
    }

    fn skipped(&self, key: Key, from: i32, to: i32, reason: SkipReason) {
        let chat_id = match key {
            Key::Channel(id) => Some(get_channel_id(id)),
            _ => None,
        };
        self.client.0.events.emit(ClientEvent::UpdatesSkipped { chat_id, from, to, reason });
    }

    /// Hands every update the tracker has released to the handlers, in order.
    fn dispatch_ready(&mut self) {
        let held_before = self.held_before;
        for (update, entities) in std::mem::take(&mut self.tracker.ready) {
            // Telegram pushes what it held again, so until a difference is read, a pushed update may be held too.
            if held_before.is_some_and(|since| difference::held(&update, since)) {
                continue;
            }
            self.dispatch(update, &entities);
        }
    }

    fn dispatch(&mut self, update: base::Update, entities: &Arc<Entities>) {
        let message = match &update {
            base::Update::UpdateNewMessage(u) => Some(&u.message),
            base::Update::UpdateNewChannelMessage(u) => Some(&u.message),
            base::Update::UpdateEditMessage(u) => Some(&u.message),
            base::Update::UpdateEditChannelMessage(u) => Some(&u.message),
            _ => None,
        };
        if let Some(message) = message {
            entities.remember_min_peers(&self.client, message);
        }
        self.client.stop_stream(&update);
        self.refresh_router();
        self.router.dispatch(&self.client, update, entities, &mut self.tasks);
    }

    /// Routes again the updates listeners were handed but dropped before reading.
    fn redeliver(&mut self) {
        self.refresh_router();
        self.router.redeliver(&self.client, &mut self.tasks);
    }

    fn refresh_router(&mut self) {
        let handlers = &self.client.0.handlers;
        let generation = handlers.generation();
        if generation != self.generation {
            (self.router, self.generation) = (handlers.current(), generation);
            self.tasks.on_error = self.router.on_error.clone();
        }
    }
}

impl Drop for Dispatcher {
    fn drop(&mut self) {
        self.client.0.handlers.listeners.close();
        *self.client.0.recover.lock().unwrap_or_else(PoisonError::into_inner) = None;
        let Ok(mut state) = self.client.0.state.lock() else { return };
        if self.same_account() {
            *state = Some(std::mem::take(&mut self.tracker.state));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{ClientEvent, Dispatcher, Entities, Incoming, Key, SkipReason, base, types};
    use crate::Client;
    use crate::dispatcher::listeners::tests::until_waiting;
    use crate::errors::{Error, RpcError};
    use crate::raw::{Identifiable, Serializable};
    use crate::session::{Session, UpdatesRx};
    use crate::storage::{Recording, Storage, UpdateState};
    use crate::types::{Message, RawUpdate};
    use crate::utils::{get_channel_id, get_peer};

    fn state() -> types::updates::State {
        types::updates::State { pts: 1, qts: 0, date: 0, seq: 0, unread_count: 0 }
    }

    #[test]
    fn failed_differences_are_reported() {
        let client = Client::offline();
        let events = client.0.events.record();
        let d = Dispatcher::new(client, &state());
        d.report(Ok(()));
        d.report(Err(Error::Disconnected));
        let got: Vec<ClientEvent> = events.try_iter().collect();
        assert!(
            matches!(&got[..], [ClientEvent::DifferenceFailed { error }] if matches!(**error, Error::Disconnected)),
            "{got:?}"
        );
    }

    #[tokio::test]
    async fn min_senders_are_cached_with_no_handlers() {
        let client = Client::offline();
        let channel = get_channel_id(9);
        let chat = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        let chat = base::InputPeer::InputPeerChannel(Box::new(chat));
        client.0.peers.lock().unwrap().insert(channel, chat.clone());
        let mut buf = Vec::new();
        (1u32 << 20).serialize(&mut buf);
        0u32.serialize(&mut buf);
        7i64.serialize(&mut buf);
        let min_user = types::User::deserialize_bare(&mut &buf[..]).unwrap();
        let entities = Arc::new(Entities::new(&client, vec![base::User::User(Box::new(min_user))], Vec::new()));
        let mut d = Dispatcher::new(client.clone(), &state());
        let sent_by_7 = || {
            let mut message = Message::bare(42, get_peer(channel), 0);
            message.from_id = Some(get_peer(7));
            let message = base::Message::Message(Box::new(message));
            let update = types::UpdateNewChannelMessage { message, pts: 0, pts_count: 0 };
            base::Update::UpdateNewChannelMessage(Box::new(update))
        };

        d.dispatch(sent_by_7(), &entities);
        let from_message = types::InputPeerUserFromMessage { peer: chat.clone(), msg_id: 42, user_id: 7 };
        let from_message = base::InputPeer::InputPeerUserFromMessage(Box::new(from_message));
        assert_eq!(client.resolve_peer(7).await.unwrap(), from_message);
        let user = types::InputUserFromMessage { peer: chat, msg_id: 42, user_id: 7 };
        let user = base::InputUser::InputUserFromMessage(Box::new(user));
        assert_eq!(client.resolve_user(&7.into()).await.unwrap(), user);

        let full = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 5 }));
        client.0.peers.lock().unwrap().insert(7, full.clone());
        d.dispatch(sent_by_7(), &entities);
        assert_eq!(client.resolve_peer(7).await.unwrap(), full);
    }

    #[tokio::test]
    async fn a_failed_channel_page_does_not_replay_the_pages_before_it() {
        let page = types::updates::ChannelDifference {
            r#final: false,
            pts: 5,
            timeout: None,
            new_messages: vec![base::Message::Message(Box::new(Message::bare(42, get_peer(get_channel_id(7)), 0)))],
            other_updates: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        let page = base::updates::ChannelDifference::ChannelDifference(Box::new(page));
        let refused = || Err(Error::Rpc(RpcError::parse(400, "CHANNEL_BROKEN")));
        let (client, _sent) = Client::scripted(vec![Ok(page.to_bytes()), refused()]);
        let seen = raw_updates(&client);
        let peer = types::InputPeerChannel { channel_id: 7, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(get_channel_id(7), base::InputPeer::InputPeerChannel(Box::new(peer)));
        let mut d = Dispatcher::new(client, &state());
        d.tracker.state.channels.insert(7, 1);
        assert!(d.channel_difference(7).await.is_err());
        settle().await;
        // The first page's message went to the handlers before the second page was asked for.
        let handled = seen.lock().unwrap().len();
        assert_eq!((d.tracker.state.channels[&7], d.tracker.ready.len(), handled), (5, 0, 1));
    }

    #[tokio::test(start_paused = true)]
    async fn a_flood_wait_on_get_difference_is_returned_not_slept() {
        let flood = Error::Rpc(RpcError::parse(420, "FLOOD_WAIT_30"));
        let (client, mut sent) = Client::scripted(vec![Err(flood)]);
        let mut d = Dispatcher::new(client, &state());
        let result = d.difference().await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.code == 420), "{result:?}");
        let id = u32::from_le_bytes(sent.try_recv().unwrap()[..4].try_into().unwrap());
        assert_eq!(id, crate::raw::functions::updates::GetDifference::CONSTRUCTOR_ID);
        assert!(sent.try_recv().is_err());
    }

    fn user(id: i64) -> base::User {
        let mut buf = Vec::new();
        0u32.serialize(&mut buf);
        0u32.serialize(&mut buf);
        id.serialize(&mut buf);
        base::User::User(Box::new(types::User::deserialize_bare(&mut &buf[..]).unwrap()))
    }

    fn channel(id: i64) -> base::Chat {
        let mut buf = Vec::new();
        0u32.serialize(&mut buf);
        0u32.serialize(&mut buf);
        id.serialize(&mut buf);
        "c".to_owned().serialize(&mut buf);
        base::ChatPhoto::ChatPhotoEmpty.serialize(&mut buf);
        0i32.serialize(&mut buf);
        base::Chat::Channel(Box::new(types::Channel::deserialize_bare(&mut &buf[..]).unwrap()))
    }

    fn new_message(pts: i32) -> base::Update {
        let message = base::Message::Message(Box::new(Message::bare(1, get_peer(get_channel_id(9)), 0)));
        base::Update::UpdateNewMessage(Box::new(types::UpdateNewMessage { message, pts, pts_count: 1 }))
    }

    fn raw_updates(client: &Client) -> Arc<Mutex<Vec<RawUpdate>>> {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        client.on_raw_update(move |_, update| {
            log.lock().unwrap().push(update);
            async { Ok(()) }
        });
        seen
    }

    async fn settle() {
        for _ in 0..20 {
            tokio::task::yield_now().await;
        }
    }

    async fn deliver(d: &mut Dispatcher, updates: base::Updates) {
        d.process(Incoming { updates, from_request: false });
        d.resolve().await;
        for (update, entities) in std::mem::take(&mut d.tracker.ready) {
            d.dispatch(update, &entities);
        }
        settle().await;
    }

    #[tokio::test]
    async fn an_update_in_a_reply_and_on_the_stream_runs_once() {
        for reply_first in [true, false] {
            let client = Client::offline();
            let seen = raw_updates(&client);
            let (tx, mut rx) = UpdatesRx::channel_with(client.0.events.clone());
            let mut d = Dispatcher::new(client.clone(), &state());
            let copy = || {
                let updates =
                    types::Updates { updates: vec![new_message(2)], users: vec![], chats: vec![], date: 0, seq: 0 };
                base::Updates::Updates(Box::new(updates))
            };
            if reply_first {
                client.feed_updates(copy());
            }
            tx.send(Incoming { updates: copy(), from_request: false }).await.unwrap();
            if !reply_first {
                d.process(rx.recv().await.unwrap());
                client.feed_updates(copy());
            }
            for _ in 0..if reply_first { 2 } else { 1 } {
                // Only a guard against a copy that never comes; nothing asserts on the time.
                let wait = rx.recv();
                let incoming = tokio::time::timeout(std::time::Duration::from_secs(5), wait).await;
                d.process(incoming.expect("the reply's copy never arrived").unwrap());
            }
            d.resolve().await;
            for (update, entities) in std::mem::take(&mut d.tracker.ready) {
                d.dispatch(update, &entities);
            }
            for _ in 0..20 {
                tokio::task::yield_now().await;
            }
            // A reply's copy only moves the counters, as for any request's answer; the stream's copy is what a
            // handler sees. Either way pts moves once and nothing is asked of getDifference.
            let handled = seen.lock().unwrap().len();
            let counted = (d.tracker.state.pts, d.tracker.due.is_empty());
            assert_eq!((handled, counted), (usize::from(!reply_first), (2, true)), "reply first: {reply_first}");
        }
    }

    #[tokio::test]
    async fn raw_updates_carry_their_users_and_chats() {
        let client = Client::offline();
        let seen = raw_updates(&client);
        let mut d = Dispatcher::new(client.clone(), &state());
        let updates = types::Updates {
            updates: vec![new_message(2)],
            users: vec![user(7), user(3)],
            chats: vec![channel(9)],
            date: 0,
            seq: 0,
        };
        deliver(&mut d, base::Updates::Updates(Box::new(updates))).await;

        let seen = seen.lock().unwrap();
        let [update] = &seen[..] else { panic!("{seen:?}") };
        assert_eq!(update.constructor_id(), types::UpdateNewMessage::CONSTRUCTOR_ID);
        assert_eq!(update.user_ids().collect::<Vec<_>>(), [3, 7]);
        assert_eq!(update.chat_ids().collect::<Vec<_>>(), [get_channel_id(9)]);
        assert_eq!(update.user(7).map(|u| u.id), Some(7));
        assert_eq!(update.chat(get_channel_id(9)).map(|c| c.id), Some(get_channel_id(9)));
        assert!(update.user(8).is_none() && update.chat(get_channel_id(8)).is_none());
        #[cfg(feature = "json")]
        {
            use crate::raw::ToJson;
            let mut json = String::new();
            update.to_json(&mut json);
            let (update, users) = (new_message(2).json(), [user(3).json(), user(7).json()]);
            let expected =
                format!(r#"{{"update":{update},"users":[{}],"chats":[{}]}}"#, users.join(","), channel(9).json());
            assert_eq!(json, expected);
        }
    }

    #[tokio::test]
    async fn short_updates_come_without_users_or_chats() {
        let client = Client::offline();
        let seen = raw_updates(&client);
        let mut d = Dispatcher::new(client.clone(), &state());
        let status = types::UpdateUserStatus { user_id: 7, status: base::UserStatus::UserStatusEmpty };
        let update = types::UpdateShort { update: base::Update::UpdateUserStatus(Box::new(status)), date: 0 };
        deliver(&mut d, base::Updates::UpdateShort(Box::new(update))).await;

        let seen = seen.lock().unwrap();
        let [update] = &seen[..] else { panic!("{seen:?}") };
        assert_eq!((update.user_ids().count(), update.chat_ids().count()), (0, 0));
        #[cfg(feature = "json")]
        {
            let mut json = String::new();
            update.to_json(&mut json);
            assert!(json.ends_with(r#","users":[],"chats":[]}"#), "{json}");
        }
    }

    #[tokio::test]
    async fn missed_updates_bring_their_users_to_raw_handlers() {
        let difference = types::updates::Difference {
            new_messages: Vec::new(),
            new_encrypted_messages: Vec::new(),
            other_updates: vec![new_message(2)],
            chats: Vec::new(),
            users: vec![user(5), user(4)],
            state: base::updates::State::State(Box::new(types::updates::State { pts: 2, ..state() })),
        };
        let difference = base::updates::Difference::Difference(Box::new(difference));
        let (client, _sent) = Client::scripted(vec![Ok(difference.to_bytes())]);
        let seen = raw_updates(&client);
        let mut d = Dispatcher::new(client.clone(), &state());
        deliver(&mut d, base::Updates::UpdatesTooLong).await;

        let seen = seen.lock().unwrap();
        let [update] = &seen[..] else { panic!("{seen:?}") };
        assert_eq!(update.user_ids().collect::<Vec<_>>(), [4, 5]);
    }

    #[tokio::test]
    async fn raw_handlers_share_one_copy_of_the_users_and_chats() {
        let client = Client::offline();
        let (first, second) = (raw_updates(&client), raw_updates(&client));
        let mut d = Dispatcher::new(client.clone(), &state());
        let updates =
            types::Updates { updates: vec![new_message(2)], users: vec![user(3)], chats: Vec::new(), date: 0, seq: 0 };
        deliver(&mut d, base::Updates::Updates(Box::new(updates))).await;

        let (first, second) = (first.lock().unwrap(), second.lock().unwrap());
        assert!(Arc::ptr_eq(&first[0].entities, &second[0].entities));
        assert_eq!(Arc::strong_count(&first[0].entities), 2);
    }

    fn dialog(channel_id: i64, pts: i32) -> base::Dialog {
        let mut buf = Vec::new();
        1u32.serialize(&mut buf);
        get_peer(get_channel_id(channel_id)).serialize(&mut buf);
        for _ in 0..7 {
            0i32.serialize(&mut buf);
        }
        // peerNotifySettings with no flags.
        0x99622c0cu32.serialize(&mut buf);
        0u32.serialize(&mut buf);
        pts.serialize(&mut buf);
        base::Dialog::Dialog(Box::new(types::Dialog::deserialize_bare(&mut &buf[..]).unwrap()))
    }

    fn in_channel(
        replies: Vec<crate::errors::Result<Vec<u8>>>,
        pts: i32,
    ) -> (Dispatcher, std::sync::mpsc::Receiver<ClientEvent>) {
        let (client, _sent) = Client::scripted(replies);
        let channel =
            base::InputPeer::InputPeerChannel(Box::new(types::InputPeerChannel { channel_id: 9, access_hash: 1 }));
        client.0.peers.lock().unwrap().insert(get_channel_id(9), channel);
        let events = client.0.events.record();
        let mut d = Dispatcher::new(client, &state());
        d.tracker.state.channels.insert(9, pts);
        d.tracker.mark(Key::Channel(9));
        (d, events)
    }

    #[tokio::test]
    async fn recovery_counts_messages_then_other_updates() {
        let bare = || base::Message::Message(Box::new(Message::bare(1, get_peer(7), 0)));
        let difference = types::updates::Difference {
            new_messages: vec![bare(), bare()],
            new_encrypted_messages: Vec::new(),
            other_updates: vec![new_message(2)],
            chats: Vec::new(),
            users: Vec::new(),
            state: base::updates::State::State(Box::new(types::updates::State { pts: 2, ..state() })),
        };
        let difference = base::updates::Difference::Difference(Box::new(difference));
        let (client, _sent) = Client::scripted(vec![Ok(difference.to_bytes())]);
        let mut d = Dispatcher::new(client, &state());
        let (reply, answer) = tokio::sync::oneshot::channel();
        d.recover(super::Recover { chats: None, reply }).await;
        assert_eq!(answer.await.unwrap(), (2, 1));
        // The updates wait for the next turn of `run` to be dispatched.
        assert_eq!(d.tracker.ready.len(), 3);
    }

    // Real time: the store thread answers outside the runtime, which a paused clock would race past.
    #[tokio::test]
    async fn stop_does_not_wait_for_a_handler_stuck_in_recover_gaps() {
        let me = vec![base::User::UserEmpty(types::UserEmpty { id: 1 })];
        let empty = base::updates::Difference::DifferenceEmpty(types::updates::DifferenceEmpty { date: 2, seq: 3 });
        let (client, _sent) = Client::scripted(vec![Ok(me.to_bytes()), Ok(empty.to_bytes())]);
        *client.0.state.lock().unwrap() = Some(UpdateState { pts: 1, date: 1, ..Default::default() });
        let (updates, rx) = UpdatesRx::channel();
        *client.0.updates.lock().await = rx;
        let (started, release) = (Arc::new(tokio::sync::Notify::new()), Arc::new(tokio::sync::Notify::new()));
        let (tx, mut answers) = tokio::sync::mpsc::unbounded_channel();
        client.on_message(crate::filters::all(), {
            let (client, started, release) = (client.clone(), started.clone(), release.clone());
            move |_| {
                let (client, started, release, tx) = (client.clone(), started.clone(), release.clone(), tx.clone());
                async move {
                    started.notify_one();
                    release.notified().await;
                    let _ = tx.send(client.recover_gaps().await);
                    Ok(())
                }
            }
        });
        let run = tokio::spawn({
            let client = client.clone();
            async move { client.run().await }
        });
        let batch = types::Updates { updates: vec![new_message(2)], users: vec![], chats: vec![], date: 0, seq: 0 };
        let incoming = Incoming { updates: base::Updates::Updates(Box::new(batch)), from_request: false };
        updates.send(incoming).await.unwrap();
        started.notified().await;

        // The handler asks only after the stop, so nothing may be left waiting for it.
        client.stop();
        release.notify_one();
        let stopped = tokio::time::timeout(std::time::Duration::from_secs(20), run).await;
        assert!(matches!(stopped, Ok(Ok(Ok(())))), "stop hung on a handler in recover_gaps: {stopped:?}");
        let got = answers.recv().await.expect("the handler never answered");
        assert!(matches!(got, Err(Error::Disconnected | Error::InvalidArgument(_))), "{got:?}");
    }

    #[tokio::test]
    async fn recovery_only_asks_for_the_named_boxes() {
        let page = types::updates::ChannelDifference {
            r#final: true,
            pts: 5,
            timeout: None,
            new_messages: vec![base::Message::Message(Box::new(Message::bare(42, get_peer(get_channel_id(9)), 0)))],
            other_updates: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        let page = base::updates::ChannelDifference::ChannelDifference(Box::new(page));
        // One reply only: asking for the common box or the unknown channel would fail the test.
        let (mut d, _events) = in_channel(vec![Ok(page.to_bytes())], 1);
        d.tracker.due.clear();
        let (reply, answer) = tokio::sync::oneshot::channel();
        d.recover(super::Recover { chats: Some(vec![get_channel_id(9), get_channel_id(77)]), reply }).await;
        assert_eq!(answer.await.unwrap(), (1, 0));
        assert_eq!(d.tracker.state.channels[&9], 5);
    }

    #[tokio::test]
    async fn recovery_skips_ids_that_are_no_channel() {
        let (client, mut sent) = Client::recording();
        let mut d = Dispatcher::new(client, &state());
        d.tracker.state.channels.insert(9, 1);
        d.tracker.due.clear();
        let (reply, answer) = tokio::sync::oneshot::channel();
        let chats = vec![i64::MAX, 7, -7, -super::CHANNEL_OFFSET + 1, i64::MIN];
        d.recover(super::Recover { chats: Some(chats), reply }).await;
        assert_eq!(answer.await.unwrap(), (0, 0));
        assert!(sent.try_recv().is_err(), "a box was fetched");
    }

    #[tokio::test]
    async fn pts_changed_refetches_the_state() {
        let fresh = types::updates::State { pts: 500, qts: 3, date: 9, seq: 4, unread_count: 0 };
        let fresh = base::updates::State::State(Box::new(fresh));
        let (client, mut sent) = Client::scripted(vec![Ok(fresh.to_bytes())]);
        let mut d = Dispatcher::new(client, &state());
        // Held for a gap under the old numbering.
        d.tracker.update(new_message(3), &Arc::default(), false);
        let short = types::UpdateShort { update: base::Update::UpdatePtsChanged, date: 0 };
        d.process(Incoming { updates: base::Updates::UpdateShort(Box::new(short)), from_request: false });
        d.resolve().await;
        assert_eq!(sent.try_recv().unwrap(), crate::raw::functions::updates::GetState {}.to_bytes());
        assert!(sent.try_recv().is_err(), "more than getState was asked");
        let s = &d.tracker.state;
        assert_eq!((s.pts, s.qts, s.date, s.seq), (500, 3, 9, 4));
        assert!(d.tracker.pending.is_empty() && !d.tracker.refetch);
        // Raw handlers still see it.
        assert!(matches!(d.tracker.ready[..], [(base::Update::UpdatePtsChanged, _)]));
    }

    #[tokio::test]
    async fn too_long_differences_report_the_skipped_range() {
        let too_long = base::updates::Difference::DifferenceTooLong(types::updates::DifferenceTooLong { pts: 50 });
        let empty = base::updates::Difference::DifferenceEmpty(types::updates::DifferenceEmpty { date: 0, seq: 0 });
        let (client, _sent) = Client::scripted(vec![Ok(too_long.to_bytes()), Ok(empty.to_bytes())]);
        let events = client.0.events.record();
        let mut d = Dispatcher::new(client, &types::updates::State { pts: 10, ..state() });
        deliver(&mut d, base::Updates::UpdatesTooLong).await;

        let got: Vec<ClientEvent> = events.try_iter().collect();
        let skipped = ClientEvent::UpdatesSkipped { chat_id: None, from: 10, to: 50, reason: SkipReason::TooLong };
        assert_eq!(format!("{got:?}"), format!("{:?}", [skipped]));
        assert_eq!(d.tracker.state.pts, 50);
    }

    #[tokio::test]
    async fn too_long_channel_differences_report_the_skipped_range() {
        let too_long = types::updates::ChannelDifferenceTooLong {
            r#final: true,
            timeout: None,
            dialog: dialog(9, 50),
            messages: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        let too_long = base::updates::ChannelDifference::ChannelDifferenceTooLong(Box::new(too_long));
        let (mut d, events) = in_channel(vec![Ok(too_long.to_bytes())], 10);
        d.resolve().await;

        let got: Vec<ClientEvent> = events.try_iter().collect();
        let chat_id = Some(get_channel_id(9));
        let skipped = ClientEvent::UpdatesSkipped { chat_id, from: 10, to: 50, reason: SkipReason::TooLong };
        assert_eq!(format!("{got:?}"), format!("{:?}", [skipped]));
        assert_eq!(d.tracker.state.channels[&9], 50);
    }

    #[tokio::test]
    async fn refused_channel_differences_report_where_they_stopped() {
        let (mut d, events) = in_channel(vec![Err(Error::Rpc(RpcError::parse(400, "CHANNEL_PRIVATE")))], 10);
        d.resolve().await;

        let got: Vec<ClientEvent> = events.try_iter().collect();
        let chat_id = Some(get_channel_id(9));
        let skipped = ClientEvent::UpdatesSkipped { chat_id, from: 10, to: 10, reason: SkipReason::Refused };
        assert!(matches!(got[..], [ClientEvent::DifferenceFailed { .. }, _]), "{got:?}");
        assert_eq!(format!("{:?}", got[1]), format!("{skipped:?}"));
    }

    #[tokio::test(start_paused = true)]
    async fn stop_ends_waiting_listeners_before_waiting_for_handlers() {
        let me = vec![base::User::UserEmpty(types::UserEmpty { id: 1 })];
        let empty = base::updates::Difference::DifferenceEmpty(types::updates::DifferenceEmpty { date: 2, seq: 3 });
        let (client, _sent) = Client::scripted(vec![Ok(me.to_bytes()), Ok(empty.to_bytes())]);
        *client.0.state.lock().unwrap() = Some(UpdateState { pts: 1, date: 1, ..Default::default() });
        let (updates, rx) = UpdatesRx::channel();
        *client.0.updates.lock().await = rx;
        let (tx, mut answers) = tokio::sync::mpsc::unbounded_channel();
        let listener = client.clone();
        client.on_message(crate::filters::all(), move |m| {
            let (client, tx) = (listener.clone(), tx.clone());
            async move {
                let _ = tx.send(client.wait_for_message(m.chat.id).await.map(|m| m.id));
                Ok(())
            }
        });
        let start = tokio::time::Instant::now();
        let run = tokio::spawn({
            let client = client.clone();
            async move { client.run().await }
        });
        let batch = types::Updates { updates: vec![new_message(2)], users: vec![], chats: vec![], date: 0, seq: 0 };
        let incoming = Incoming { updates: base::Updates::Updates(Box::new(batch)), from_request: false };
        updates.send(incoming).await.unwrap();
        until_waiting(&client.0.handlers.listeners.messages).await;

        client.stop();
        assert!(run.await.unwrap().is_ok());
        let got = answers.recv().await.expect("the handler never answered");
        assert!(matches!(got, Err(Error::ListenerStopped)), "{got:?}");
        assert!(start.elapsed() < std::time::Duration::from_secs(300), "stop waited for the listener's timeout");
    }

    fn recording_client() -> (Client, Arc<Recording>) {
        let engine = Arc::new(Recording::default());
        let (mut client, _sent) = Client::scripted(Vec::new());
        Arc::get_mut(&mut client.0).unwrap().set_store(engine.clone());
        (client, engine)
    }

    /// Runs a dispatcher over one update that moves pts to 2, then closes the update channel.
    async fn run_until_disconnected(client: &Client, forget_account: bool) -> bool {
        let mut d = Dispatcher::new(client.clone(), &state());
        let (updates, mut rx) = UpdatesRx::channel();
        let short = types::UpdateShort { update: new_message(2), date: 0 };
        let incoming = Incoming { updates: base::Updates::UpdateShort(Box::new(short)), from_request: false };
        updates.send(incoming).await.unwrap();
        drop(updates);
        if forget_account {
            client.forget_account();
        }
        d.run(&mut rx).await
    }

    #[tokio::test]
    async fn a_lost_connection_saves_the_update_state() {
        let (client, engine) = recording_client();
        let stopped = run_until_disconnected(&client, false).await;
        // Read before the client is dropped, which would save it anyway.
        let saved = engine.load_state().await.unwrap().and_then(|s| s.update);
        assert!(!stopped);
        assert_eq!(saved.map(|s| s.pts), Some(2));
    }

    #[tokio::test]
    async fn a_lost_connection_saves_nothing_for_a_forgotten_account() {
        let (client, engine) = recording_client();
        let stopped = run_until_disconnected(&client, true).await;
        assert!(!stopped);
        assert!(!engine.calls().contains(&"save_state"), "{:?}", engine.calls());
    }

    #[tokio::test]
    async fn a_run_dropped_between_difference_pages_has_handled_what_it_hands_back() {
        let state = types::updates::State { pts: 50, qts: 0, date: 5, seq: 0, unread_count: 0 };
        let slice = types::updates::DifferenceSlice {
            new_messages: vec![base::Message::Message(Box::new(Message::bare(42, get_peer(7), 0)))],
            new_encrypted_messages: Vec::new(),
            other_updates: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
            intermediate_state: base::updates::State::State(Box::new(state)),
        };
        let page = base::updates::Difference::DifferenceSlice(Box::new(slice));
        // The first page is answered, the second never is.
        let (session, mut sent) = Session::answering(vec![Ok(page.to_bytes())]);
        let client = Client::with_session(session);
        let seen = raw_updates(&client);
        let (_updates, mut rx) = UpdatesRx::channel_with(client.0.events.clone());
        let mut d = Dispatcher::resume(client.clone(), UpdateState { pts: 10, date: 1, ..Default::default() });
        tokio::select! {
            _ = d.run(&mut rx) => panic!("run ended"),
            _ = async { sent.recv().await; sent.recv().await } => {}
        }
        drop(d);
        settle().await;
        let handed_back = client.0.state.lock().unwrap().as_ref().map(|s| s.pts);
        assert_eq!((handed_back, seen.lock().unwrap().len()), (Some(50), 1));
    }

    #[tokio::test]
    async fn a_message_a_dropped_listener_took_still_reaches_the_handlers() {
        use crate::dispatcher::listeners::tests::new_message;
        let client = Client::offline();
        let handled = Arc::new(Mutex::new(Vec::new()));
        let log = handled.clone();
        client.on_message(crate::filters::all(), move |m| {
            log.lock().unwrap().push(m.id);
            async { Ok(()) }
        });
        let mut d = Dispatcher::new(client.clone(), &state());
        let mut listen = std::future::IntoFuture::into_future(client.listen::<Message>());
        tokio::select! {
            biased;
            _ = &mut listen => panic!("nothing was offered"),
            () = std::future::ready(()) => {}
        }
        // The listener takes the message, then is dropped before it reads it, as a `select!` that picks another
        // branch does.
        d.dispatch(new_message(5, 7), &Arc::default());
        drop(listen);
        let (_updates, mut rx) = UpdatesRx::channel_with(client.0.events.clone());
        let reached = async {
            while handled.lock().unwrap().is_empty() {
                tokio::task::yield_now().await;
            }
        };
        tokio::select! {
            _ = d.run(&mut rx) => panic!("run ended"),
            _ = tokio::time::timeout(std::time::Duration::from_secs(5), reached) => {}
        }
        assert_eq!(*handled.lock().unwrap(), [5]);
    }

    #[tokio::test(start_paused = true)]
    async fn with_a_handler_cap_updates_wait_in_the_queue_once_the_waiting_room_is_full() {
        use super::tasks::MAX_WAITING_HANDLERS;
        let mut client = Client::offline();
        Arc::get_mut(&mut client.0).unwrap().handler_permits = Some(Arc::new(tokio::sync::Semaphore::new(1)));
        // The first handler never ends, so every later one waits for its slot.
        client.on_raw_update(|_, _| std::future::pending::<crate::errors::Result<()>>());
        let mut d = Dispatcher::new(client.clone(), &state());
        let (updates, mut rx) = UpdatesRx::channel_with(client.0.events.clone());
        let status = || {
            let status = types::UpdateUserStatus { user_id: 7, status: base::UserStatus::UserStatusEmpty };
            let update = types::UpdateShort { update: base::Update::UpdateUserStatus(Box::new(status)), date: 0 };
            Incoming { updates: base::Updates::UpdateShort(Box::new(update)), from_request: false }
        };
        let offered = 3 * MAX_WAITING_HANDLERS;
        let feed = async {
            let mut taken = 0;
            while taken < offered {
                let send = tokio::time::timeout(std::time::Duration::from_secs(1), updates.send(status()));
                if send.await.is_err() {
                    break;
                }
                taken += 1;
            }
            taken
        };
        let taken = tokio::select! {
            _ = d.run(&mut rx) => panic!("run ended"),
            taken = feed => taken,
        };
        // One running, the waiting room, and what the update queue holds (8 in tests).
        let room = MAX_WAITING_HANDLERS..=1 + MAX_WAITING_HANDLERS + 8 + 1;
        assert!(room.contains(&taken), "the dispatcher took {taken} of {offered} updates");
    }
}
