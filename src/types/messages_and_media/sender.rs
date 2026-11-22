use std::sync::Arc;

use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::{Chat, PeerId, User};

/// Who a message is from: a user, or a chat it was sent on behalf of (a channel post, an anonymous admin, a
/// message sent as a channel).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Sender {
    /// A user or bot sent it as themselves.
    User(Box<User>),
    /// It was sent on behalf of this chat.
    Chat(Arc<Chat>),
}

impl Sender {
    pub(crate) fn parse(peer: &base::Peer, entities: &Entities) -> Sender {
        match peer {
            base::Peer::PeerUser(p) => Sender::User(Box::new(User::parse(p.user_id, entities))),
            peer => Sender::Chat(Arc::new(Chat::parse(peer, entities, false))),
        }
    }

    /// The sending user, `None` when a chat sent it.
    pub fn user(&self) -> Option<&User> {
        match self {
            Sender::User(user) => Some(user),
            Sender::Chat(_) => None,
        }
    }

    /// The chat it was sent on behalf of, `None` when a user sent it as themselves.
    pub fn chat(&self) -> Option<&Arc<Chat>> {
        match self {
            Sender::User(_) => None,
            Sender::Chat(chat) => Some(chat),
        }
    }

    /// The Bot-API-style id of the user or chat.
    pub fn id(&self) -> i64 {
        match self {
            Sender::User(user) => user.id,
            Sender::Chat(chat) => chat.id,
        }
    }

    /// The typed id of the user or chat.
    pub fn peer_id(&self) -> PeerId {
        PeerId::from_bot_api(self.id())
    }
}

#[cfg(test)]
impl ReplyTo {
    /// A plain reply to message `id` in the same chat.
    pub(crate) fn message(id: i32) -> ReplyTo {
        ReplyTo::Message { message_id: Some(id), checklist_task_id: None, poll_option_id: None }
    }
}

/// What a message replies to.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum ReplyTo {
    /// A message. The quoted part, the thread and a reply into another chat are on the message itself:
    /// [`Message::quote`](crate::types::Message::quote), [`message_thread_id`](crate::types::Message::message_thread_id)
    /// and [`external_reply`](crate::types::Message::external_reply).
    Message {
        /// Id of the replied message in this chat. `None` when it is in another chat, see `external_reply`.
        message_id: Option<i32>,
        /// The checklist task the reply is about.
        checklist_task_id: Option<i32>,
        /// The poll option the reply is about, the same bytes as [`PollOption::data`](crate::types::PollOption).
        poll_option_id: Option<Vec<u8>>,
    },
    /// A story.
    Story {
        /// The chat that posted the story: a user, or a `-100…` channel id.
        chat_id: i64,
        /// Id of the story.
        story_id: i32,
    },
}
