use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::PeerRef;
use crate::utils::{CHANNEL_OFFSET, chats_of, input_user};

pub(crate) enum ChatPeer {
    Group(i64),
    Channel(base::InputChannel),
}

impl Client {
    pub async fn resolve_peer(&self, chat_id: impl Into<PeerRef>) -> Result<base::InputPeer> {
        Ok(self.resolve(&chat_id.into()).await?.1)
    }

    pub(crate) async fn resolve(&self, chat: &PeerRef) -> Result<(i64, base::InputPeer)> {
        let chat_id = match chat {
            PeerRef::Id(id) => *id,
            PeerRef::Me => return Ok((self.0.me.lock().unwrap().id, base::InputPeer::InputPeerSelf)),
            PeerRef::Username(name) => self.resolve_username(name).await?,
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

    async fn resolve_id(&self, chat_id: i64) -> Result<base::InputPeer> {
        if let Some(peer) = self.0.peers.lock().unwrap().get(&chat_id) {
            return Ok(peer.clone());
        }
        if chat_id == self.0.me.lock().unwrap().id {
            return Ok(base::InputPeer::InputPeerSelf);
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

    pub(crate) async fn resolve_chat(&self, chat: &PeerRef) -> Result<ChatPeer> {
        match self.resolve(chat).await?.1 {
            base::InputPeer::InputPeerChat(p) => Ok(ChatPeer::Group(p.chat_id)),
            base::InputPeer::InputPeerChannel(p) => {
                let channel = types::InputChannel { channel_id: p.channel_id, access_hash: p.access_hash };
                Ok(ChatPeer::Channel(base::InputChannel::InputChannel(Box::new(channel))))
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
    use crate::raw::functions::{contacts, messages};
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
}
