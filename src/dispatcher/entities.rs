use std::collections::HashMap;

use crate::Client;
use crate::raw::{base, types};
use crate::utils::{get_channel_id, get_peer_id};

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
                base::Chat::Community(c) => (get_channel_id(c.id), None),
                base::Chat::CommunityForbidden(c) => (get_channel_id(c.id), None),
                _ => continue,
            };
            if let Some(peer) = peer {
                peers.insert(id, base::InputPeer::InputPeerChannel(Box::new(peer)));
            }
            entities.chats.insert(id, chat);
        }
        entities
    }

    pub(crate) fn remember_min_sender(&self, client: &Client, update: &base::Update) {
        let message = match update {
            base::Update::UpdateNewMessage(u) => &u.message,
            base::Update::UpdateNewChannelMessage(u) => &u.message,
            base::Update::UpdateEditMessage(u) => &u.message,
            base::Update::UpdateEditChannelMessage(u) => &u.message,
            _ => return,
        };
        let base::Message::Message(message) = message else { return };
        let Some(from) = &message.from_id else { return };
        let min = match from {
            base::Peer::PeerUser(p) => matches!(self.users.get(&p.user_id), Some(base::User::User(u)) if u.min),
            base::Peer::PeerChannel(p) => {
                matches!(self.chats.get(&get_channel_id(p.channel_id)), Some(base::Chat::Channel(c)) if c.min)
            }
            base::Peer::PeerChat(_) => false,
        };
        if !min {
            return;
        }
        let id = get_peer_id(from);
        let mut peers = client.0.peers.lock().unwrap();
        if matches!(peers.get(&id), Some(base::InputPeer::InputPeerUser(_) | base::InputPeer::InputPeerChannel(_))) {
            return;
        }
        let peer = match &message.peer_id {
            base::Peer::PeerChat(p) => {
                base::InputPeer::InputPeerChat(Box::new(types::InputPeerChat { chat_id: p.chat_id }))
            }
            chat => match peers.get(&get_peer_id(chat)) {
                Some(peer) => peer.clone(),
                None => return,
            },
        };
        let msg_id = message.id;
        let sender = match from {
            base::Peer::PeerUser(p) => {
                let user = types::InputPeerUserFromMessage { peer, msg_id, user_id: p.user_id };
                base::InputPeer::InputPeerUserFromMessage(Box::new(user))
            }
            base::Peer::PeerChannel(p) => {
                let channel = types::InputPeerChannelFromMessage { peer, msg_id, channel_id: p.channel_id };
                base::InputPeer::InputPeerChannelFromMessage(Box::new(channel))
            }
            base::Peer::PeerChat(_) => return,
        };
        peers.insert(id, sender);
    }
}
