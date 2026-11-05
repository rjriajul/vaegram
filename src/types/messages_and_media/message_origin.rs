use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::{Chat, User};

/// Where a forwarded message originally came from.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MessageOrigin {
    /// Sent by a user whose account is visible.
    User {
        /// Unix time the original message was sent.
        date: i32,
        /// The user who sent the message.
        sender_user: User,
    },
    /// Sent by a user who hides their account when forwarded. Only their name is known.
    HiddenUser {
        /// Unix time the original message was sent.
        date: i32,
        /// Name the sender chose to show.
        sender_user_name: String,
    },
    /// A message imported from another app's chat export; `sender_user_name` is the name the export gave the sender.
    Import {
        /// Unix time the original message was sent.
        date: i32,
        /// Name of the sender given by the export.
        sender_user_name: String,
    },
    /// Sent on behalf of a chat, such as an anonymous group admin.
    Chat {
        /// Unix time the original message was sent.
        date: i32,
        /// The chat the message was sent on behalf of.
        sender_chat: Chat,
        /// Signature of the author, `None` when there is none.
        author_signature: Option<String>,
    },
    /// Posted in a channel, with the id of the original post.
    Channel {
        /// Unix time the original message was sent.
        date: i32,
        /// The channel the message was posted in.
        chat: Chat,
        /// Id of the original post in the channel.
        message_id: i32,
        /// Signature of the post author, `None` when there is none.
        author_signature: Option<String>,
    },
}

impl MessageOrigin {
    pub(crate) fn parse(header: &base::MessageFwdHeader, entities: &Entities) -> Option<MessageOrigin> {
        let base::MessageFwdHeader::MessageFwdHeader(h) = header;
        let date = h.date;
        if h.imported {
            let sender_user_name = h.from_name.clone().or_else(|| h.post_author.clone()).unwrap_or_default();
            return Some(MessageOrigin::Import { date, sender_user_name });
        }
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
