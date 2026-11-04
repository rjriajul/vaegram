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
                // A min user's hash is only good for the message that carried it.
                if let (false, Some(access_hash)) = (u.min, u.access_hash) {
                    let names = active(&u.username, &u.usernames);
                    peers.remember(u.id, access_hash, &names, u.phone.as_deref());
                }
                entities.users.insert(u.id, user);
            }
        }
        for chat in chats {
            let id = match &chat {
                base::Chat::Chat(c) => -c.id,
                base::Chat::ChatForbidden(c) => -c.id,
                base::Chat::Channel(c) => {
                    let id = get_channel_id(c.id);
                    if let (false, Some(access_hash)) = (c.min, c.access_hash) {
                        peers.remember(id, access_hash, &active(&c.username, &c.usernames), None);
                    }
                    id
                }
                base::Chat::ChannelForbidden(c) => {
                    let id = get_channel_id(c.id);
                    peers.remember(id, c.access_hash, &[], None);
                    id
                }
                base::Chat::Community(c) => get_channel_id(c.id),
                base::Chat::CommunityForbidden(c) => get_channel_id(c.id),
                _ => continue,
            };
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
            base::Peer::PeerChat(p) => base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: p.chat_id }),
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

/// The names a user or channel can be found by: its username and its active collectible ones, each once.
fn active<'a>(username: &'a Option<String>, usernames: &'a Option<Vec<base::Username>>) -> Vec<&'a str> {
    let mut names: Vec<&str> = username.iter().map(String::as_str).collect();
    for base::Username::Username(u) in usernames.iter().flatten() {
        if u.active && !names.iter().any(|n| n.eq_ignore_ascii_case(&u.username)) {
            names.push(&u.username);
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use crate::raw::{base, types};

    fn name(username: &str, active: bool) -> base::Username {
        let u =
            types::Username { editable: false, active, deletable: false, expired: false, username: username.into() };
        base::Username::Username(u.into())
    }

    #[test]
    fn every_active_username_is_kept_once() {
        let main = Some("Alice".to_owned());
        let others = Some(vec![name("alice", true), name("alice_nft", true), name("old", false)]);
        assert_eq!(super::active(&main, &others), ["Alice", "alice_nft"]);
        assert_eq!(super::active(&None, &others), ["alice", "alice_nft"]);
        assert_eq!(super::active(&main, &None), ["Alice"]);
    }
}
