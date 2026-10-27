use std::collections::HashMap;

use crate::Client;
use crate::raw::{base, types};
use crate::utils::get_channel_id;

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
