use crate::dispatcher::Entities;
use crate::enums::ChatType;
use crate::raw::base;
use crate::types::{Community, User};
use crate::utils::{get_channel_id, get_peer, get_peer_id};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chat {
    pub id: i64,
    pub kind: ChatType,
    pub title: Option<String>,
    pub username: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub bio: Option<String>,
    pub description: Option<String>,
    pub members_count: Option<i32>,
    /// The bot answering join request queries; only admins get it from `get_chat`.
    pub guard_bot: Option<User>,
    /// The community the chat or bot belongs to; only `get_chat` fills it.
    pub community: Option<Community>,
    pub is_direct_messages: bool,
    pub parent_chat: Option<Box<Chat>>,
    pub is_forum: bool,
}

impl Chat {
    pub(crate) fn parse(peer: &base::Peer, entities: &Entities, is_post: bool) -> Chat {
        let id = get_peer_id(peer);
        let mut chat = Chat {
            id,
            kind: Chat::kind_of(peer, entities, is_post),
            title: None,
            username: None,
            first_name: None,
            last_name: None,
            bio: None,
            description: None,
            members_count: None,
            guard_bot: None,
            community: None,
            is_direct_messages: false,
            parent_chat: None,
            is_forum: false,
        };
        match peer {
            base::Peer::PeerUser(_) => {
                if let Some(base::User::User(u)) = entities.users.get(&id) {
                    chat.username = u.username.clone();
                    chat.first_name = u.first_name.clone();
                    chat.last_name = u.last_name.clone();
                }
            }
            base::Peer::PeerChat(_) => {
                chat.title = match entities.chats.get(&id) {
                    Some(base::Chat::Chat(c)) => Some(c.title.clone()),
                    Some(base::Chat::ChatForbidden(c)) => Some(c.title.clone()),
                    _ => None,
                };
            }
            base::Peer::PeerChannel(_) => match entities.chats.get(&id) {
                Some(base::Chat::Channel(c)) => {
                    chat.title = Some(c.title.clone());
                    chat.username = c.username.clone();
                    chat.is_direct_messages = c.monoforum;
                    // A direct messages chat links back to its channel through the same field.
                    if c.monoforum {
                        chat.parent_chat = c
                            .linked_monoforum_id
                            .map(|parent| Box::new(Chat::parse(&get_peer(get_channel_id(parent)), entities, true)));
                    }
                    chat.is_forum = c.forum;
                }
                Some(base::Chat::ChannelForbidden(c)) => {
                    chat.title = Some(c.title.clone());
                    chat.is_direct_messages = c.monoforum;
                }
                _ => {}
            },
        }
        chat
    }

    pub(crate) fn kind_of(peer: &base::Peer, entities: &Entities, is_post: bool) -> ChatType {
        let id = get_peer_id(peer);
        match peer {
            base::Peer::PeerUser(_) => match entities.users.get(&id) {
                Some(base::User::User(u)) if u.bot => ChatType::Bot,
                _ => ChatType::Private,
            },
            base::Peer::PeerChat(_) => ChatType::Group,
            base::Peer::PeerChannel(_) => match entities.chats.get(&id) {
                Some(base::Chat::Channel(c)) if c.megagroup => ChatType::Supergroup,
                Some(base::Chat::Channel(_)) => ChatType::Channel,
                Some(base::Chat::ChannelForbidden(c)) if c.megagroup || !c.broadcast => ChatType::Supergroup,
                Some(base::Chat::ChannelForbidden(_)) => ChatType::Channel,
                _ if is_post => ChatType::Channel,
                _ => ChatType::Supergroup,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Chat;
    use crate::dispatcher::Entities;
    use crate::enums::ChatType;
    use crate::raw::{base, types};
    use crate::utils::{get_channel_id, get_peer};

    #[test]
    fn a_forbidden_channel_keeps_its_kind() {
        let kind = |broadcast, megagroup| {
            let forbidden = types::ChannelForbidden {
                broadcast,
                megagroup,
                monoforum: false,
                id: 5,
                access_hash: 1,
                title: "t".into(),
                until_date: None,
            };
            let chats = [(get_channel_id(5), base::Chat::ChannelForbidden(Box::new(forbidden)))].into();
            let entities = Entities { users: Default::default(), chats };
            let chat = Chat::parse(&get_peer(get_channel_id(5)), &entities, false);
            (chat.kind, chat.title)
        };
        assert_eq!(kind(true, false), (ChatType::Channel, Some("t".into())));
        assert_eq!(kind(false, true), (ChatType::Supergroup, Some("t".into())));
    }
}
