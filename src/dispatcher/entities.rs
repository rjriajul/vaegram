use std::collections::HashMap;

use crate::Client;
use crate::raw::{base, types};
use crate::utils::{channel_id_of, get_channel_id, get_peer_id};

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

    /// Remembers, for every min user or channel `message` names, the message it was seen in: Telegram sends a min
    /// peer without a hash this account may use, so the message is the only way to address it. Covers the sender,
    /// the forward and external-reply origins, the inline bot, mentions and the users of a join, add or remove.
    /// A peer already known with its own hash keeps it.
    pub(crate) fn remember_min_peers(&self, client: &Client, message: &base::Message) {
        let (msg_id, chat) = match message {
            base::Message::Message(m) => (m.id, &m.peer_id),
            base::Message::MessageService(m) => (m.id, &m.peer_id),
            base::Message::MessageEmpty(_) => return,
        };
        let mut any = false;
        named_peers(message, |id| any |= self.is_min(id));
        if !any {
            return;
        }
        let mut peers = client.0.peers.lock().unwrap();
        let chat = match chat {
            base::Peer::PeerChat(p) => base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: p.chat_id }),
            chat => match peers.get(&get_peer_id(chat)) {
                Some(peer) => peer.clone(),
                None => return,
            },
        };
        named_peers(message, |id| {
            if !self.is_min(id)
                || matches!(
                    peers.get(&id),
                    Some(base::InputPeer::InputPeerUser(_) | base::InputPeer::InputPeerChannel(_))
                )
            {
                return;
            }
            let peer = chat.clone();
            let input = if id > 0 {
                let user = types::InputPeerUserFromMessage { peer, msg_id, user_id: id };
                base::InputPeer::InputPeerUserFromMessage(Box::new(user))
            } else {
                let channel = types::InputPeerChannelFromMessage { peer, msg_id, channel_id: channel_id_of(id) };
                base::InputPeer::InputPeerChannelFromMessage(Box::new(channel))
            };
            peers.insert(id, input);
        });
    }

    /// Whether Telegram described the user or channel with this Bot-API id as min.
    fn is_min(&self, id: i64) -> bool {
        if id > 0 {
            matches!(self.users.get(&id), Some(base::User::User(u)) if u.min)
        } else {
            matches!(self.chats.get(&id), Some(base::Chat::Channel(c)) if c.min)
        }
    }
}

/// Calls `f` with the Bot-API id of every peer `message` names that may come as a min peer.
fn named_peers(message: &base::Message, mut f: impl FnMut(i64)) {
    fn origin(header: &Option<base::MessageFwdHeader>) -> Option<&base::Peer> {
        match header {
            Some(base::MessageFwdHeader::MessageFwdHeader(h)) => h.from_id.as_ref(),
            None => None,
        }
    }
    fn replied(reply_to: &Option<base::MessageReplyHeader>) -> Option<&base::Peer> {
        match reply_to {
            Some(base::MessageReplyHeader::MessageReplyHeader(h)) => origin(&h.reply_from),
            _ => None,
        }
    }
    match message {
        base::Message::Message(m) => {
            for peer in [m.from_id.as_ref(), origin(&m.fwd_from), replied(&m.reply_to)].into_iter().flatten() {
                f(get_peer_id(peer));
            }
            m.via_bot_id.into_iter().for_each(&mut f);
            for entity in m.entities.iter().flatten() {
                if let base::MessageEntity::MessageEntityMentionName(e) = entity {
                    f(e.user_id);
                }
            }
        }
        base::Message::MessageService(m) => {
            for peer in [m.from_id.as_ref(), replied(&m.reply_to)].into_iter().flatten() {
                f(get_peer_id(peer));
            }
            match &m.action {
                base::MessageAction::MessageActionChatAddUser(a) => a.users.iter().copied().for_each(f),
                base::MessageAction::MessageActionChatDeleteUser(a) => f(a.user_id),
                base::MessageAction::MessageActionChatJoinedByLink(a) => f(a.inviter_id),
                _ => {}
            }
        }
        base::Message::MessageEmpty(_) => {}
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
    use super::Entities;
    use crate::Client;
    use crate::raw::{Serializable, base, types};
    use crate::types::Message;
    use crate::utils::{get_channel_id, get_peer};

    fn name(username: &str, active: bool) -> base::Username {
        let u =
            types::Username { editable: false, active, deletable: false, expired: false, username: username.into() };
        base::Username::Username(u.into())
    }

    fn min_user(id: i64) -> base::User {
        let mut buf = Vec::new();
        (1u32 << 20).serialize(&mut buf);
        0u32.serialize(&mut buf);
        id.serialize(&mut buf);
        base::User::User(Box::new(types::User::deserialize_bare(&mut &buf[..]).unwrap()))
    }

    fn origin(id: i64) -> base::MessageFwdHeader {
        base::MessageFwdHeader::MessageFwdHeader(Box::new(types::MessageFwdHeader {
            imported: false,
            saved_out: false,
            from_id: Some(get_peer(id)),
            from_name: None,
            date: 0,
            channel_post: None,
            post_author: None,
            saved_from_peer: None,
            saved_from_msg_id: None,
            saved_from_id: None,
            saved_from_name: None,
            saved_date: None,
            psa_type: None,
        }))
    }

    fn reply_from(id: i64) -> base::MessageReplyHeader {
        base::MessageReplyHeader::MessageReplyHeader(Box::new(types::MessageReplyHeader {
            reply_to_scheduled: false,
            forum_topic: false,
            quote: false,
            reply_to_ephemeral: false,
            reply_to_msg_id: Some(1),
            reply_to_peer_id: Some(get_peer(get_channel_id(77))),
            reply_from: Some(origin(id)),
            reply_media: None,
            reply_to_top_id: None,
            quote_text: None,
            quote_entities: None,
            quote_offset: None,
            todo_item_id: None,
            poll_option: None,
            quote_rich: None,
        }))
    }

    /// A client that knows the supergroup -1000000000009, and the peer it is reached by.
    fn in_group() -> (Client, base::InputPeer) {
        let client = Client::offline();
        let group =
            base::InputPeer::InputPeerChannel(Box::new(types::InputPeerChannel { channel_id: 9, access_hash: 1 }));
        client.0.peers.lock().unwrap().insert(get_channel_id(9), group.clone());
        (client, group)
    }

    fn seen_in(group: &base::InputPeer, user_id: i64) -> base::InputPeer {
        let user = types::InputPeerUserFromMessage { peer: group.clone(), msg_id: 42, user_id };
        base::InputPeer::InputPeerUserFromMessage(Box::new(user))
    }

    #[tokio::test]
    async fn min_users_named_anywhere_in_a_message_are_reached_through_it() {
        let (client, group) = in_group();
        let entities = Entities::new(&client, (1..=5).map(min_user).collect(), Vec::new());
        let mut message = Message::bare(42, get_peer(get_channel_id(9)), 0);
        message.from_id = Some(get_peer(1));
        message.fwd_from = Some(origin(2));
        message.reply_to = Some(reply_from(3));
        message.via_bot_id = Some(4);
        message.message = "Eve".into();
        let mention = types::MessageEntityMentionName { offset: 0, length: 3, user_id: 5 };
        message.entities = Some(vec![base::MessageEntity::MessageEntityMentionName(Box::new(mention))]);
        entities.remember_min_peers(&client, &base::Message::Message(Box::new(message)));
        for id in 1..=5 {
            assert_eq!(client.resolve_peer(id).await.unwrap(), seen_in(&group, id), "user {id}");
        }
    }

    #[tokio::test]
    async fn min_users_of_a_service_message_are_reached_through_it() {
        let (client, group) = in_group();
        let entities = Entities::new(&client, (1..=3).map(min_user).collect(), Vec::new());
        let service = |from: i64, action| {
            base::Message::MessageService(Box::new(types::MessageService {
                out: false,
                mentioned: false,
                media_unread: false,
                reactions_are_possible: false,
                silent: false,
                post: false,
                legacy: false,
                id: 42,
                from_id: Some(get_peer(from)),
                peer_id: get_peer(get_channel_id(9)),
                saved_peer_id: None,
                reply_to: None,
                date: 0,
                action,
                reactions: None,
                ttl_period: None,
            }))
        };
        let added = types::MessageActionChatAddUser { users: vec![2] };
        entities
            .remember_min_peers(&client, &service(1, base::MessageAction::MessageActionChatAddUser(Box::new(added))));
        let joined = types::MessageActionChatJoinedByLink { inviter_id: 3 };
        entities.remember_min_peers(&client, &service(3, base::MessageAction::MessageActionChatJoinedByLink(joined)));
        for id in 1..=3 {
            assert_eq!(client.resolve_peer(id).await.unwrap(), seen_in(&group, id), "user {id}");
        }
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
