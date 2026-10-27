use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use crate::Client;
use crate::errors::Result;
use crate::handlers::{BoxFuture, Handler};
use crate::raw::{base, functions, types};
use crate::session::Incoming;
use crate::types::Message;
use crate::utils::get_channel_id;

const CHANNEL_DIFFERENCE_LIMIT: i32 = 100;

#[derive(Default)]
pub(crate) struct Entities {
    pub(crate) users: HashMap<i64, base::User>,
    pub(crate) chats: HashMap<i64, base::Chat>,
}

#[derive(Debug, PartialEq)]
enum Gap {
    Apply,
    Skip,
    Fill,
}

fn check(local: i32, remote: i32, count: i32) -> Gap {
    match (local + count).cmp(&remote) {
        Ordering::Equal => Gap::Apply,
        Ordering::Greater => Gap::Skip,
        Ordering::Less => Gap::Fill,
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

pub(crate) struct Dispatcher {
    client: Client,
    pts: i32,
    qts: i32,
    date: i32,
    seq: i32,
    channels: HashMap<i64, i32>,
    ready: Vec<(base::Update, Arc<Entities>)>,
}

impl Dispatcher {
    pub(crate) fn new(client: Client, state: &types::updates::State) -> Self {
        let mut d = Dispatcher { client, pts: 0, qts: 0, date: 0, seq: 0, channels: HashMap::new(), ready: Vec::new() };
        d.set_state(state);
        d
    }

    pub(crate) async fn handle(&mut self, incoming: Incoming) {
        if let Err(e) = self.process(incoming).await {
            eprintln!("vaegram: failed to fetch missed updates: {e}");
        }
        for (update, entities) in std::mem::take(&mut self.ready) {
            self.dispatch(update, &entities);
        }
    }

    fn set_state(&mut self, s: &types::updates::State) {
        (self.pts, self.qts, self.date, self.seq) = (s.pts, s.qts, s.date, s.seq);
    }

    fn emit(&mut self, update: base::Update, entities: &Arc<Entities>) {
        self.ready.push((update, entities.clone()));
    }

    async fn process(&mut self, incoming: Incoming) -> Result<()> {
        let quiet = incoming.from_request;
        match incoming.updates {
            base::Updates::UpdatesTooLong => self.difference().await,
            base::Updates::UpdateShortMessage(u) => self.short(u.pts, u.pts_count, quiet).await,
            base::Updates::UpdateShortChatMessage(u) => self.short(u.pts, u.pts_count, quiet).await,
            base::Updates::UpdateShortSentMessage(u) => self.short(u.pts, u.pts_count, true).await,
            base::Updates::UpdateShort(u) => self.update(u.update, &Arc::default(), quiet).await,
            base::Updates::Updates(u) => self.batch(u.updates, u.users, u.chats, (u.date, u.seq, u.seq), quiet).await,
            base::Updates::UpdatesCombined(u) => {
                self.batch(u.updates, u.users, u.chats, (u.date, u.seq_start, u.seq), quiet).await
            }
        }
    }

    async fn short(&mut self, pts: i32, pts_count: i32, quiet: bool) -> Result<()> {
        match check(self.pts, pts, pts_count) {
            Gap::Skip => Ok(()),
            Gap::Apply if quiet => {
                self.pts = pts;
                Ok(())
            }
            _ => self.difference().await,
        }
    }

    async fn batch(
        &mut self,
        updates: Vec<base::Update>,
        users: Vec<base::User>,
        chats: Vec<base::Chat>,
        (date, seq_start, seq): (i32, i32, i32),
        quiet: bool,
    ) -> Result<()> {
        if seq_start != 0 {
            match check(self.seq, seq_start, 1) {
                Gap::Skip => return Ok(()),
                Gap::Fill => return self.difference().await,
                Gap::Apply => {}
            }
        }
        let entities = Arc::new(self.entities(users, chats));
        for update in updates {
            self.update(update, &entities, quiet).await?;
        }
        if seq != 0 {
            (self.seq, self.date) = (seq, date);
        }
        Ok(())
    }

    async fn update(&mut self, update: base::Update, entities: &Arc<Entities>, quiet: bool) -> Result<()> {
        if matches!(update, base::Update::UpdateChannelTooLong(_)) || channel_of(&update).is_some() {
            return self.channel_update(update, entities, quiet).await;
        }
        if let Some((pts, count)) = update.pts() {
            match check(self.pts, pts, count) {
                Gap::Skip => return Ok(()),
                Gap::Fill => return self.difference().await,
                Gap::Apply => self.pts = pts,
            }
        } else if let Some(qts) = update.qts() {
            match check(self.qts, qts, 1) {
                Gap::Skip => return Ok(()),
                Gap::Fill => return self.difference().await,
                Gap::Apply => self.qts = qts,
            }
        }
        if !quiet {
            self.emit(update, entities);
        }
        Ok(())
    }

    async fn channel_update(&mut self, update: base::Update, entities: &Arc<Entities>, quiet: bool) -> Result<()> {
        if let base::Update::UpdateChannelTooLong(u) = &update {
            return self.channel_difference(u.channel_id).await;
        }
        let (Some(channel_id), Some((pts, count))) = (channel_of(&update), update.pts()) else {
            return Ok(());
        };
        if let Some(&local) = self.channels.get(&channel_id) {
            match check(local, pts, count) {
                Gap::Skip => return Ok(()),
                Gap::Fill if self.channel_difference(channel_id).await.is_ok() => return Ok(()),
                _ => {}
            }
        }
        self.channels.insert(channel_id, pts);
        if !quiet {
            self.emit(update, entities);
        }
        Ok(())
    }

    async fn difference(&mut self) -> Result<()> {
        loop {
            let request = functions::updates::GetDifference {
                pts: self.pts,
                pts_limit: None,
                pts_total_limit: None,
                date: self.date,
                qts: self.qts,
                qts_limit: None,
            };
            let (messages, others, users, chats, state, done) = match self.client.invoke(&request).await? {
                base::updates::Difference::DifferenceEmpty(d) => {
                    (self.date, self.seq) = (d.date, d.seq);
                    return Ok(());
                }
                base::updates::Difference::DifferenceTooLong(d) => {
                    self.pts = d.pts;
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
            let entities = Arc::new(self.entities(users, chats));
            for message in messages {
                self.emit(new_message(message), &entities);
            }
            for update in others {
                if matches!(update, base::Update::UpdateChannelTooLong(_)) || channel_of(&update).is_some() {
                    self.channel_update(update, &entities, false).await?;
                } else {
                    self.emit(update, &entities);
                }
            }
            let base::updates::State::State(state) = state;
            self.set_state(&state);
            if done {
                return Ok(());
            }
        }
    }

    async fn channel_difference(&mut self, channel_id: i64) -> Result<()> {
        let Some(&local) = self.channels.get(&channel_id) else { return Ok(()) };
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
                    let entities = Arc::new(self.entities(d.users, d.chats));
                    for message in d.messages {
                        self.emit(new_channel_message(message), &entities);
                    }
                    if let base::Dialog::Dialog(dialog) = &d.dialog {
                        pts = dialog.pts.unwrap_or(pts);
                    }
                    break;
                }
                base::updates::ChannelDifference::ChannelDifference(d) => {
                    let d = *d;
                    let entities = Arc::new(self.entities(d.users, d.chats));
                    for message in d.new_messages {
                        self.emit(new_channel_message(message), &entities);
                    }
                    for update in d.other_updates {
                        self.emit(update, &entities);
                    }
                    pts = d.pts;
                    if d.r#final {
                        break;
                    }
                }
            }
        }
        self.channels.insert(channel_id, pts);
        Ok(())
    }

    fn entities(&self, users: Vec<base::User>, chats: Vec<base::Chat>) -> Entities {
        let mut peers = self.client.0.peers.lock().unwrap();
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

    fn dispatch(&self, update: base::Update, entities: &Entities) {
        let handlers = self.client.0.handlers.read().unwrap().clone();
        let message = match &update {
            base::Update::UpdateNewMessage(u) => Some(&u.message),
            base::Update::UpdateNewChannelMessage(u) => Some(&u.message),
            _ => None,
        };
        let mut message = match message {
            Some(base::Message::Message(m)) => Some(Message::parse(&self.client, m, entities)),
            _ => None,
        };
        for handler in handlers {
            match handler {
                Handler::RawUpdate(callback) => spawn(callback(self.client.clone(), update.clone())),
                Handler::Message(filter, callback) => {
                    if message.as_ref().is_some_and(|m| filter.check(m)) {
                        spawn(callback(message.take().unwrap()));
                    }
                }
            }
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
    use super::{Gap, check};

    #[test]
    fn finds_gaps() {
        assert_eq!(check(10, 11, 1), Gap::Apply);
        assert_eq!(check(10, 13, 3), Gap::Apply);
        assert_eq!(check(10, 10, 1), Gap::Skip);
        assert_eq!(check(10, 9, 1), Gap::Skip);
        assert_eq!(check(10, 12, 1), Gap::Fill);
    }
}
