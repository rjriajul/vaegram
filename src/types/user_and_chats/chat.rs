use crate::dispatcher::Entities;
use crate::enums::ChatType;
use crate::raw::base;
use crate::types::User;
use crate::utils::get_peer_id;

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
}

impl Chat {
    pub(crate) fn parse(peer: &base::Peer, entities: &Entities, is_post: bool) -> Chat {
        let id = get_peer_id(peer);
        let mut chat = Chat {
            id,
            kind: ChatType::Private,
            title: None,
            username: None,
            first_name: None,
            last_name: None,
            bio: None,
            description: None,
            members_count: None,
            guard_bot: None,
        };
        match peer {
            base::Peer::PeerUser(_) => {
                if let Some(base::User::User(u)) = entities.users.get(&id) {
                    chat.kind = if u.bot { ChatType::Bot } else { ChatType::Private };
                    chat.username = u.username.clone();
                    chat.first_name = u.first_name.clone();
                    chat.last_name = u.last_name.clone();
                }
            }
            base::Peer::PeerChat(_) => {
                chat.kind = ChatType::Group;
                chat.title = match entities.chats.get(&id) {
                    Some(base::Chat::Chat(c)) => Some(c.title.clone()),
                    Some(base::Chat::ChatForbidden(c)) => Some(c.title.clone()),
                    _ => None,
                };
            }
            base::Peer::PeerChannel(_) => {
                chat.kind = if is_post { ChatType::Channel } else { ChatType::Supergroup };
                match entities.chats.get(&id) {
                    Some(base::Chat::Channel(c)) => {
                        chat.kind = if c.megagroup { ChatType::Supergroup } else { ChatType::Channel };
                        chat.title = Some(c.title.clone());
                        chat.username = c.username.clone();
                    }
                    Some(base::Chat::ChannelForbidden(c)) => chat.title = Some(c.title.clone()),
                    _ => {}
                }
            }
        }
        chat
    }
}
