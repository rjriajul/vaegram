use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::storage::{USERNAME_TTL, input_peer};
use crate::types::PeerRef;
use crate::utils::{CHANNEL_OFFSET, chats_of, input_user};

pub(crate) enum ChatPeer {
    Group(i64),
    Channel(base::InputChannel),
}

impl Client {
    /// Returns the TL peer for a chat id, username or invite link, with its access hash, for the raw requests sent
    /// with [`invoke`](Client::invoke).
    ///
    /// Together with `invoke` this is the raw escape hatch: the friendly API never needs it.
    pub async fn resolve_peer(&self, chat_id: impl Into<PeerRef>) -> Result<base::InputPeer> {
        Ok(self.resolve(&chat_id.into()).await?.1)
    }

    pub(crate) async fn resolve(&self, chat: &PeerRef) -> Result<(i64, base::InputPeer)> {
        let chat_id = match chat {
            PeerRef::Id(id) => *id,
            PeerRef::Me => return Ok((self.0.me.lock().unwrap().id, base::InputPeer::InputPeerSelf)),
            PeerRef::Username(name) => self.username_id(name).await?,
            PeerRef::Invite(hash) => {
                let request = functions::messages::CheckChatInvite { hash: String::from(&**hash) };
                let chat = match self.invoke(&request).await? {
                    base::ChatInvite::ChatInviteAlready(c) => c.chat,
                    base::ChatInvite::ChatInvitePeek(c) => c.chat,
                    base::ChatInvite::ChatInvite(_) => return Err(Error::ChatNotJoined),
                };
                let entities = Entities::new(self, Vec::new(), vec![chat]);
                entities.chats.into_keys().next().ok_or(Error::Unexpected("messages.checkChatInvite sent no chat"))?
            }
        };
        Ok((chat_id, self.resolve_id(chat_id).await?))
    }

    /// A username's peer: from memory or the store while the name is fresh, else from Telegram.
    async fn username_id(&self, name: &str) -> Result<i64> {
        let name = name.trim_start_matches('@').to_ascii_lowercase();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
        let fresh = |updated: i64| now.saturating_sub(updated) <= USERNAME_TTL.as_secs() as i64;
        let cached = self.0.peers.lock().unwrap().by_username(&name);
        if let Some((id, _)) = cached.filter(|&(_, updated)| fresh(updated)) {
            return Ok(id);
        }
        match self.0.store.get_peer_by_username(name.clone()).await {
            Ok(Some(record)) => {
                if fresh(record.updated) && input_peer(record.id, record.access_hash).is_some() {
                    let id = record.id;
                    self.0.peers.lock().unwrap().load([record]);
                    return Ok(id);
                }
            }
            Ok(None) => {}
            Err(e) => self
                .0
                .events
                .storage_failed(e.kind(), format_args!("couldn't look @{name} up in storage, asking Telegram: {e}")),
        }
        self.resolve_username(&name).await
    }

    async fn resolve_id(&self, chat_id: i64) -> Result<base::InputPeer> {
        if let Some(peer) = self.known_peer(chat_id).await {
            return Ok(peer);
        }

        let peer = if chat_id > 0 {
            let input = types::InputUser { user_id: chat_id, access_hash: 0 };
            let users = self
                .invoke(&functions::users::GetUsers { id: vec![base::InputUser::InputUser(Box::new(input))] })
                .await?;
            let access_hash = users
                .iter()
                .find_map(|u| match u {
                    base::User::User(u) if u.id == chat_id => u.access_hash,
                    _ => None,
                })
                .ok_or(Error::PeerNotFound(chat_id))?;
            base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: chat_id, access_hash }))
        } else if chat_id <= -CHANNEL_OFFSET {
            let channel_id = -chat_id - CHANNEL_OFFSET;
            let input = types::InputChannel { channel_id, access_hash: 0 };
            let chats = self
                .invoke(&functions::channels::GetChannels {
                    id: vec![base::InputChannel::InputChannel(Box::new(input))],
                })
                .await?;
            let access_hash = chats_of(chats)
                .iter()
                .find_map(|c| match c {
                    base::Chat::Channel(c) if c.id == channel_id => c.access_hash,
                    base::Chat::ChannelForbidden(c) if c.id == channel_id => Some(c.access_hash),
                    _ => None,
                })
                .ok_or(Error::PeerNotFound(chat_id))?;
            base::InputPeer::InputPeerChannel(Box::new(types::InputPeerChannel { channel_id, access_hash }))
        } else {
            base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: -chat_id })
        };

        self.0.peers.lock().unwrap().insert(chat_id, peer.clone());
        Ok(peer)
    }

    /// The peer of a user or channel this account can address without a request: from memory, or saved in the
    /// store by an earlier run (one hop to the store thread). A failing engine is reported and counts as unknown.
    pub(crate) async fn known_peer(&self, chat_id: i64) -> Option<base::InputPeer> {
        if let Some(peer) = self.0.peers.lock().unwrap().get(&chat_id) {
            return Some(peer.clone());
        }
        if chat_id == self.0.me.lock().unwrap().id {
            return Some(base::InputPeer::InputPeerSelf);
        }
        // Basic groups need no hash.
        if chat_id > 0 || chat_id <= -CHANNEL_OFFSET {
            match self.0.store.get_peer(chat_id).await {
                Ok(Some(record)) => {
                    if let Some(peer) = input_peer(record.id, record.access_hash) {
                        self.0.peers.lock().unwrap().load([record]);
                        return Some(peer);
                    }
                }
                Ok(None) => {}
                Err(e) => self.0.events.storage_failed(
                    e.kind(),
                    format_args!("couldn't look chat {chat_id} up in storage, asking Telegram: {e}"),
                ),
            }
        }
        None
    }

    pub(crate) async fn peer_id(&self, chat: PeerRef) -> Result<i64> {
        match chat {
            PeerRef::Id(id) => Ok(id),
            PeerRef::Me => Ok(self.0.me.lock().unwrap().id),
            chat => Ok(self.resolve(&chat).await?.0),
        }
    }

    /// Resolves one at a time, so it's meant for cached ids: each unknown username costs a request.
    pub(crate) async fn resolve_peers(
        &self,
        chats: impl IntoIterator<Item = impl Into<PeerRef>>,
    ) -> Result<Vec<base::InputPeer>> {
        let mut peers = Vec::new();
        for chat in chats {
            peers.push(self.resolve_peer(chat).await?);
        }
        Ok(peers)
    }

    pub(crate) async fn resolve_users(
        &self,
        users: impl IntoIterator<Item = impl Into<PeerRef>>,
    ) -> Result<Vec<base::InputUser>> {
        let mut input = Vec::new();
        for user in users {
            input.push(self.resolve_user(&user.into()).await?);
        }
        Ok(input)
    }

    pub(crate) async fn resolve_user(&self, user: &PeerRef) -> Result<base::InputUser> {
        let (id, peer) = self.resolve(user).await?;
        input_user(peer).ok_or(Error::PeerNotFound(id))
    }

    /// Gives each mention the input user Telegram wants in place of its bare user id. A mention of a user this
    /// account has never met is dropped and its text kept, rather than the whole message failing (a helper bot
    /// captioning a file with the requester's `tg://user` link). Each user is looked up once per call.
    pub(crate) async fn resolve_mentions(
        &self,
        entities: Vec<base::MessageEntity>,
    ) -> Result<Vec<base::MessageEntity>> {
        use base::MessageEntity as E;
        let mut known: HashMap<i64, Option<base::InputUser>> = HashMap::new();
        let mut resolved = Vec::with_capacity(entities.len());
        for entity in entities {
            let (offset, length, id) = match &entity {
                E::MessageEntityMentionName(m) => (m.offset, m.length, m.user_id),
                E::InputMessageEntityMentionName(m) => match &m.user_id {
                    base::InputUser::InputUser(u) => (m.offset, m.length, u.user_id),
                    _ => {
                        resolved.push(entity);
                        continue;
                    }
                },
                _ => {
                    resolved.push(entity);
                    continue;
                }
            };
            let user_id = match known.get(&id) {
                Some(user) => user.clone(),
                None => {
                    let user = match self.resolve_user(&id.into()).await {
                        Ok(user) => Some(user),
                        Err(Error::PeerNotFound(_)) => None,
                        Err(e) if e.is("USER_ID_INVALID") || e.is("PEER_ID_INVALID") => None,
                        Err(e) => return Err(e),
                    };
                    known.insert(id, user.clone());
                    user
                }
            };
            if let Some(user_id) = user_id {
                let mention = types::InputMessageEntityMentionName { offset, length, user_id };
                resolved.push(E::InputMessageEntityMentionName(Box::new(mention)));
            }
        }
        Ok(resolved)
    }

    pub(crate) async fn resolve_chat(&self, chat: &PeerRef) -> Result<ChatPeer> {
        match self.resolve(chat).await?.1 {
            base::InputPeer::InputPeerChat(p) => Ok(ChatPeer::Group(p.chat_id)),
            base::InputPeer::InputPeerChannel(p) => {
                let channel = types::InputChannel { channel_id: p.channel_id, access_hash: p.access_hash };
                Ok(ChatPeer::Channel(base::InputChannel::InputChannel(Box::new(channel))))
            }
            // A channel only seen as a min sender: reached through the message it was seen in.
            base::InputPeer::InputPeerChannelFromMessage(p) => {
                let channel =
                    types::InputChannelFromMessage { peer: p.peer, msg_id: p.msg_id, channel_id: p.channel_id };
                Ok(ChatPeer::Channel(base::InputChannel::InputChannelFromMessage(Box::new(channel))))
            }
            _ => Err(Error::InvalidArgument("chat_id is a user, not a group or channel")),
        }
    }

    pub(crate) async fn resolve_channel(&self, chat: &PeerRef) -> Result<base::InputChannel> {
        match self.resolve_chat(chat).await? {
            ChatPeer::Channel(channel) => Ok(channel),
            ChatPeer::Group(_) => Err(Error::InvalidArgument("only supergroups and channels have that")),
        }
    }

    pub(crate) async fn full_chat(&self, chat: ChatPeer) -> Result<(base::ChatFull, Entities)> {
        let base::messages::ChatFull::ChatFull(full) = match chat {
            ChatPeer::Group(chat_id) => self.invoke(&functions::messages::GetFullChat { chat_id }).await?,
            ChatPeer::Channel(channel) => self.invoke(&functions::channels::GetFullChannel { channel }).await?,
        };
        let types::messages::ChatFull { full_chat, chats, users } = *full;
        Ok((full_chat, Entities::new(self, users, chats)))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::functions::{contacts, messages, users};
    use crate::raw::{Identifiable, Serializable, base, types};
    use crate::types::PeerRef;
    use crate::utils::get_channel_id;

    #[tokio::test]
    async fn resolves_every_kind_of_name() {
        let (client, mut rx) = Client::recording();
        client.0.me.lock().unwrap().id = 1;
        let channel = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        let channel = base::InputPeer::InputPeerChannel(Box::new(channel));
        client.0.peers.lock().unwrap().insert(get_channel_id(9), channel.clone());
        let group = base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 5 });

        let offline = [
            (PeerRef::from("me"), (1, base::InputPeer::InputPeerSelf)),
            (PeerRef::from(1), (1, base::InputPeer::InputPeerSelf)),
            (PeerRef::from(-5), (-5, group)),
            (PeerRef::from("https://t.me/c/9/20"), (get_channel_id(9), channel.clone())),
            (PeerRef::from("-1000000000009"), (get_channel_id(9), channel)),
        ];
        for (chat, expected) in offline {
            assert_eq!(client.resolve(&chat).await.unwrap(), expected, "{chat:?}");
        }
        assert!(rx.try_recv().is_err());

        let online = [
            ("@durov", contacts::ResolveUsername::CONSTRUCTOR_ID),
            ("t.me/durov/5", contacts::ResolveUsername::CONSTRUCTOR_ID),
            ("t.me/+AbC", messages::CheckChatInvite::CONSTRUCTOR_ID),
            ("t.me/joinchat/AbC", messages::CheckChatInvite::CONSTRUCTOR_ID),
        ];
        for (chat, constructor) in online {
            assert!(client.resolve_peer(chat).await.is_err());
            let body = rx.try_recv().expect("no request sent");
            assert_eq!(u32::from_le_bytes(body[..4].try_into().unwrap()), constructor, "{chat}");
        }
    }

    #[tokio::test]
    async fn an_unjoined_invite_link_is_its_own_error() {
        let invite = base::ChatInvite::ChatInvite(Box::new(types::ChatInvite {
            channel: true,
            broadcast: false,
            public: false,
            megagroup: true,
            request_needed: false,
            verified: false,
            scam: false,
            fake: false,
            can_refulfill_subscription: false,
            title: "Private".into(),
            about: None,
            photo: base::Photo::PhotoEmpty(types::PhotoEmpty { id: 0 }),
            participants_count: 3,
            participants: None,
            color: 0,
            subscription_pricing: None,
            subscription_form_id: None,
            bot_verification: None,
        }));
        let (client, _sent) = Client::scripted(vec![Ok(invite.to_bytes())]);
        let result = client.resolve_peer("t.me/+AbC").await;
        assert!(matches!(result, Err(Error::ChatNotJoined)), "{result:?}");
    }

    #[tokio::test]
    async fn a_failing_store_falls_back_to_asking_telegram() {
        let (mut client, mut sent) = Client::scripted(vec![Err(Error::Unexpected("offline"))]);
        let engine = std::sync::Arc::new(crate::storage::Recording::failing());
        std::sync::Arc::get_mut(&mut client.0).unwrap().set_store(engine.clone());
        let result = client.resolve_peer(777).await;
        let body = sent.try_recv().expect("no request sent");
        assert_eq!(u32::from_le_bytes(body[..4].try_into().unwrap()), users::GetUsers::CONSTRUCTOR_ID);
        assert!(matches!(result, Err(Error::Unexpected("offline"))), "{result:?}");
        assert_eq!(engine.calls(), ["get_peer_by_id"]);
    }

    #[tokio::test]
    async fn a_failing_store_sends_names_and_numbers_to_telegram() {
        let offline = || Err(Error::Unexpected("offline"));
        let (mut client, mut sent) = Client::scripted(vec![offline(), offline()]);
        let engine = std::sync::Arc::new(crate::storage::Recording::failing());
        std::sync::Arc::get_mut(&mut client.0).unwrap().set_store(engine.clone());
        let events = client.0.events.record();
        let by_name = client.resolve_peer("@alice").await;
        let by_phone = client.resolve_phone("+1 555 0100").await;
        assert!(matches!(by_name, Err(Error::Unexpected("offline"))), "{by_name:?}");
        assert!(matches!(by_phone, Err(Error::Unexpected("offline"))), "{by_phone:?}");
        let constructor = |body: Vec<u8>| u32::from_le_bytes(body[..4].try_into().unwrap());
        assert_eq!(constructor(sent.try_recv().unwrap()), contacts::ResolveUsername::CONSTRUCTOR_ID);
        assert_eq!(constructor(sent.try_recv().unwrap()), contacts::ResolvePhone::CONSTRUCTOR_ID);
        assert_eq!(engine.calls(), ["get_peer_by_username", "get_peer_by_phone_number"]);
        let reported: Vec<String> = events
            .try_iter()
            .map(|e| match e {
                crate::enums::ClientEvent::StorageFailed { error } => error.to_string(),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(
            reported,
            [
                "couldn't look @alice up in storage, asking Telegram: store is down",
                "couldn't look a phone number up in storage, asking Telegram: store is down",
            ]
        );
    }

    fn user_named(id: i64, username: &str) -> base::User {
        let mut buf = Vec::new();
        // flags: access_hash (bit 0) and username (bit 3)
        0b1001u32.serialize(&mut buf);
        0u32.serialize(&mut buf);
        id.serialize(&mut buf);
        7i64.serialize(&mut buf);
        username.to_owned().serialize(&mut buf);
        base::User::User(Box::new(types::User::deserialize_bare(&mut &buf[..]).unwrap()))
    }

    #[tokio::test]
    async fn a_fresh_username_is_resolved_without_a_request() {
        let (client, mut sent) = Client::scripted(Vec::new());
        crate::dispatcher::Entities::new(&client, vec![user_named(100, "Alice")], Vec::new());
        let (id, _) = client.resolve(&PeerRef::Username("@alice".into())).await.unwrap();
        assert_eq!(id, 100);
        assert!(sent.try_recv().is_err(), "a request went out");
    }

    #[tokio::test]
    async fn a_stale_username_is_looked_up_again() {
        let (client, mut sent) = Client::scripted(vec![Err(Error::Unexpected("offline"))]);
        let mut old = crate::storage::PeerRecord::new(100, 7);
        (old.usernames, old.updated) = (vec!["alice".into()], 1);
        client.0.peers.lock().unwrap().load([old]);
        let _ = client.resolve(&PeerRef::Username("alice".into())).await;
        assert_eq!(
            sent.try_recv().map(|b| u32::from_le_bytes(b[..4].try_into().unwrap())).ok(),
            Some(contacts::ResolveUsername::CONSTRUCTOR_ID)
        );
    }

    #[tokio::test]
    async fn a_stale_name_confirmed_by_telegram_is_fresh_again() {
        let peer = base::Peer::PeerUser(types::PeerUser { user_id: 100 });
        let resolved = types::contacts::ResolvedPeer { peer, chats: Vec::new(), users: vec![user_named(100, "alice")] };
        let reply = base::contacts::ResolvedPeer::ResolvedPeer(Box::new(resolved)).to_bytes();
        let (client, mut sent) = Client::scripted(vec![Ok(reply)]);
        let mut old = crate::storage::PeerRecord::new(100, 7);
        let nine_hours_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(9 * 60 * 60);
        let then = nine_hours_ago.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        (old.usernames, old.updated) = (vec!["alice".into()], then);
        client.0.peers.lock().unwrap().load([old]);

        for _ in 0..2 {
            let (id, _) = client.resolve(&PeerRef::Username("alice".into())).await.unwrap();
            assert_eq!(id, 100);
        }
        assert!(sent.try_recv().is_ok(), "the stale name was looked up");
        assert!(sent.try_recv().is_err(), "the confirmed name was looked up again");
    }

    #[tokio::test]
    async fn a_min_channel_is_reached_through_its_message() {
        let (client, mut sent) = Client::scripted(vec![Err(Error::Unexpected("offline"))]);
        let group = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        let group = base::InputPeer::InputPeerChannel(Box::new(group));
        let seen = types::InputPeerChannelFromMessage { peer: group.clone(), msg_id: 42, channel_id: 55 };
        let seen = base::InputPeer::InputPeerChannelFromMessage(Box::new(seen));
        client.0.peers.lock().unwrap().insert(get_channel_id(55), seen);

        let result = client.get_chat(get_channel_id(55)).await;
        assert!(matches!(result, Err(Error::Unexpected("offline"))), "{result:?}");
        let channel = types::InputChannelFromMessage { peer: group, msg_id: 42, channel_id: 55 };
        let channel = base::InputChannel::InputChannelFromMessage(Box::new(channel));
        assert_eq!(sent.try_recv().unwrap(), crate::raw::functions::channels::GetFullChannel { channel }.to_bytes());
    }
}
