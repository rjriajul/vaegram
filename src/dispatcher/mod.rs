mod difference;
mod entities;
pub(crate) mod router;
mod tasks;
mod tracker;

use std::sync::Arc;
use std::time::Duration;

use tokio::time::Instant;

pub(crate) use entities::Entities;
pub(crate) use router::{Router, Routes};
use tasks::Tasks;
use tracker::{Key, Tracker};

use crate::Client;
use crate::enums::ClientEvent;
use crate::errors::Result;
use crate::raw::{base, types};
use crate::session::{Incoming, UpdatesRx};
use crate::storage::state::UpdateState;

const SAVE_EVERY: Duration = Duration::from_secs(10);

pub(crate) struct Dispatcher {
    client: Client,
    tracker: Tracker,
    saved_at: Instant,
    saved: (UpdateState, usize),
    router: Arc<Router>,
    generation: u64,
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
        let router = client.0.handlers.current();
        let mut tasks = Tasks::new(client.0.ordered, client.0.handler_permits.clone());
        tasks.on_error = router.on_error.clone();
        tasks.events = client.0.events.clone();
        Dispatcher { client, tracker, saved_at: Instant::now(), saved: Default::default(), router, generation, tasks }
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
                if now != self.saved {
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
                    self.client.0.save_state(&self.tracker.state).await;
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
            self.tracker.recover(&result, key);
            self.report(result);
        }
    }

    fn report(&self, result: Result<()>) {
        if let Err(error) = result {
            eprintln!("vaegram: failed to fetch missed updates: {error}");
            self.client.0.events.emit(ClientEvent::DifferenceFailed { error });
        }
    }

    fn dispatch(&mut self, update: base::Update, entities: &Entities) {
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
        if let Ok(mut state) = self.client.0.state.lock() {
            *state = Some(std::mem::take(&mut self.tracker.state));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ClientEvent, Dispatcher, Entities, base, types};
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::{Identifiable, Serializable};
    use crate::types::Message;
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
        let entities = Entities::new(&client, vec![base::User::User(Box::new(min_user))], Vec::new());
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
}
