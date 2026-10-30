mod difference;
mod entities;
pub(crate) mod router;
mod tasks;
mod tracker;

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

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
use crate::storage::state::UpdateState;
use crate::utils::get_channel_id;

const SAVE_EVERY: Duration = Duration::from_secs(10);

pub(crate) struct Dispatcher {
    client: Client,
    tracker: Tracker,
    saved_at: Instant,
    saved: (UpdateState, usize),
    router: Arc<Router>,
    generation: u64,
    account: u64,
    tasks: Tasks,
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

    fn with(client: Client, mut tracker: Tracker) -> Self {
        tracker.touch(Instant::now());
        let generation = client.0.handlers.generation();
        let account = client.0.account.load(Ordering::Relaxed);
        let router = client.0.handlers.current();
        let mut tasks = Tasks::new(client.0.ordered, client.0.handler_permits.clone());
        tasks.on_error = router.on_error.clone();
        tasks.events = client.0.events.clone();
        let saved = Default::default();
        Dispatcher { client, tracker, saved_at: Instant::now(), saved, router, generation, account, tasks }
    }

    fn same_account(&self) -> bool {
        self.client.0.account.load(Ordering::Relaxed) == self.account
    }

    /// `true` when `Client::stop` ended it, after the running handlers finished; `false` when the connection did.
    pub(crate) async fn run(&mut self, updates: &mut UpdatesRx) -> bool {
        let mut stop = self.client.0.stop.subscribe();
        self.resolve().await;
        loop {
            for (update, entities) in std::mem::take(&mut self.tracker.ready) {
                self.dispatch(update, &entities);
            }
            // Saved every SAVE_EVERY when something moved, and on drop, so a crash replays at most that much.
            if self.saved_at.elapsed() >= SAVE_EVERY {
                self.saved_at = Instant::now();
                let now = (self.tracker.state.clone(), self.client.0.peers.lock().unwrap().len());
                if now != self.saved && self.same_account() {
                    self.client.0.save_state_in_background(&now.0);
                    self.saved = now;
                }
            }
            let deadline = self.tracker.deadline();
            let next = async {
                match deadline {
                    Some(deadline) => tokio::time::timeout_at(deadline, updates.recv()).await,
                    None => Ok(updates.recv().await),
                }
            };
            let next = tokio::select! {
                biased;
                // The watch guard `wait_for` returns isn't Send, so it is dropped before awaiting the handlers.
                _ = async { drop(stop.wait_for(|stopped| *stopped).await) } => {
                    self.tasks.finish().await;
                    if self.same_account() {
                        self.client.0.save_state(&self.tracker.state).await;
                    }
                    return true;
                }
                next = next => next,
            };
            match next {
                Ok(Some(incoming)) => self.process(incoming),
                Ok(None) => return false,
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
            let skipped = self.tracker.recover(&result, key);
            self.report(result);
            for s in skipped {
                self.skipped(s.key, s.from, s.to, s.reason);
            }
        }
    }

    fn report(&self, result: Result<()>) {
        if let Err(error) = result {
            self.client.0.events.emit(ClientEvent::DifferenceFailed { error });
        }
    }

    fn skipped(&self, key: Key, from: i32, to: i32, reason: SkipReason) {
        let chat_id = match key {
            Key::Channel(id) => Some(get_channel_id(id)),
            _ => None,
        };
        self.client.0.events.emit(ClientEvent::UpdatesSkipped { chat_id, from, to, reason });
    }

    fn dispatch(&mut self, update: base::Update, entities: &Arc<Entities>) {
        entities.remember_min_sender(&self.client, &update);
        self.client.stop_stream(&update);
        let handlers = &self.client.0.handlers;
        let generation = handlers.generation();
        if generation != self.generation {
            (self.router, self.generation) = (handlers.current(), generation);
            self.tasks.on_error = self.router.on_error.clone();
        }
        self.router.dispatch(&self.client, update, entities, &mut self.tasks);
    }
}

impl Drop for Dispatcher {
    fn drop(&mut self) {
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
    use crate::errors::{Error, RpcError};
    use crate::raw::{Identifiable, Serializable};
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
        assert!(matches!(got[..], [ClientEvent::DifferenceFailed { error: Error::Disconnected }]), "{got:?}");
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
        let refused = || Err(Error::Rpc(RpcError::new(400, "CHANNEL_BROKEN")));
        let (client, _sent) = Client::scripted(vec![Ok(page.to_bytes()), refused()]);
        let peer = types::InputPeerChannel { channel_id: 7, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(get_channel_id(7), base::InputPeer::InputPeerChannel(Box::new(peer)));
        let mut d = Dispatcher::new(client, &state());
        d.tracker.state.channels.insert(7, 1);
        assert!(d.channel_difference(7).await.is_err());
        assert_eq!((d.tracker.state.channels[&7], d.tracker.ready.len()), (5, 1));
    }

    #[tokio::test(start_paused = true)]
    async fn a_flood_wait_on_get_difference_is_returned_not_slept() {
        let flood = Error::Rpc(RpcError::new(420, "FLOOD_WAIT_30"));
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

    async fn deliver(d: &mut Dispatcher, updates: base::Updates) {
        d.process(Incoming { updates, from_request: false });
        d.resolve().await;
        for (update, entities) in std::mem::take(&mut d.tracker.ready) {
            d.dispatch(update, &entities);
        }
        for _ in 0..20 {
            tokio::task::yield_now().await;
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
        let (mut d, events) = in_channel(vec![Err(Error::Rpc(RpcError::new(400, "CHANNEL_PRIVATE")))], 10);
        d.resolve().await;

        let got: Vec<ClientEvent> = events.try_iter().collect();
        let chat_id = Some(get_channel_id(9));
        let skipped = ClientEvent::UpdatesSkipped { chat_id, from: 10, to: 10, reason: SkipReason::Refused };
        assert!(matches!(got[..], [ClientEvent::DifferenceFailed { .. }, _]), "{got:?}");
        assert_eq!(format!("{:?}", got[1]), format!("{skipped:?}"));
    }
}
