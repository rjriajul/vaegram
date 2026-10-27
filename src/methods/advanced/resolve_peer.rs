use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::utils::CHANNEL_OFFSET;

pub(crate) enum ChatPeer {
    Group,
    Channel(base::InputChannel),
}

impl Client {
    pub async fn resolve_peer(&self, chat_id: i64) -> Result<base::InputPeer> {
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
            let chats = match chats {
                base::messages::Chats::Chats(c) => c.chats,
                base::messages::Chats::ChatsSlice(c) => c.chats,
            };
            let access_hash = chats
                .iter()
                .find_map(|c| match c {
                    base::Chat::Channel(c) if c.id == channel_id => c.access_hash,
                    base::Chat::ChannelForbidden(c) if c.id == channel_id => Some(c.access_hash),
                    _ => None,
                })
                .ok_or(Error::PeerNotFound(chat_id))?;
            base::InputPeer::InputPeerChannel(Box::new(types::InputPeerChannel { channel_id, access_hash }))
        } else {
            base::InputPeer::InputPeerChat(Box::new(types::InputPeerChat { chat_id: -chat_id }))
        };

        self.0.peers.lock().unwrap().insert(chat_id, peer.clone());
        Ok(peer)
    }

    pub(crate) async fn resolve_user(&self, user_id: i64) -> Result<base::InputUser> {
        match self.resolve_peer(user_id).await? {
            base::InputPeer::InputPeerUser(p) => {
                let user = types::InputUser { user_id: p.user_id, access_hash: p.access_hash };
                Ok(base::InputUser::InputUser(Box::new(user)))
            }
            base::InputPeer::InputPeerSelf => Ok(base::InputUser::InputUserSelf),
            _ => Err(Error::PeerNotFound(user_id)),
        }
    }

    pub(crate) async fn resolve_chat(&self, chat_id: i64) -> Result<ChatPeer> {
        match self.resolve_peer(chat_id).await? {
            base::InputPeer::InputPeerChat(_) => Ok(ChatPeer::Group),
            base::InputPeer::InputPeerChannel(p) => {
                let channel = types::InputChannel { channel_id: p.channel_id, access_hash: p.access_hash };
                Ok(ChatPeer::Channel(base::InputChannel::InputChannel(Box::new(channel))))
            }
            _ => Err(Error::InvalidArgument("chat_id is a user, not a group or channel")),
        }
    }

    pub(crate) async fn resolve_channel(&self, chat_id: i64) -> Result<base::InputChannel> {
        match self.resolve_chat(chat_id).await? {
            ChatPeer::Channel(channel) => Ok(channel),
            ChatPeer::Group => Err(Error::InvalidArgument("only supergroups and channels have that")),
        }
    }
}
