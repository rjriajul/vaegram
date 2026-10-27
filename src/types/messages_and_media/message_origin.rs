use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::{Chat, User};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageOrigin {
    User { date: i32, sender_user: User },
    HiddenUser { date: i32, sender_user_name: String },
    Chat { date: i32, sender_chat: Chat, author_signature: Option<String> },
    Channel { date: i32, chat: Chat, message_id: i32, author_signature: Option<String> },
}

impl MessageOrigin {
    pub(crate) fn parse(header: &base::MessageFwdHeader, entities: &Entities) -> Option<MessageOrigin> {
        let base::MessageFwdHeader::MessageFwdHeader(h) = header;
        let date = h.date;
        Some(match (&h.from_id, h.channel_post) {
            (Some(base::Peer::PeerUser(p)), _) => {
                MessageOrigin::User { date, sender_user: User::parse(p.user_id, entities) }
            }
            (Some(peer), Some(message_id)) => MessageOrigin::Channel {
                date,
                chat: Chat::parse(peer, entities, true),
                message_id,
                author_signature: h.post_author.clone(),
            },
            (Some(peer), None) => MessageOrigin::Chat {
                date,
                sender_chat: Chat::parse(peer, entities, false),
                author_signature: h.post_author.clone(),
            },
            (None, _) => MessageOrigin::HiddenUser { date, sender_user_name: h.from_name.clone()? },
        })
    }
}
