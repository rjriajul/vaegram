use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::DirectMessagesTopic;

/// A direct messages conversation with one sender in a channel, with its unread state.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct DirectMessagesChatTopic {
    /// Who the conversation is with.
    pub topic: DirectMessagesTopic,
    /// Id of the latest message in the conversation.
    pub top_message_id: i32,
    /// Number of unread messages in the conversation.
    pub unread_messages_count: i32,
    /// Number of unread reactions in the conversation.
    pub unread_reactions_count: i32,
    /// Id of the last incoming message that was read.
    pub last_read_inbox_message_id: i32,
    /// Id of the last outgoing message that was read.
    pub last_read_outbox_message_id: i32,
    /// Whether the conversation was marked as unread by hand.
    pub unread_mark: bool,
    /// Whether the sender is exempt from paying for messages.
    pub can_send_unpaid_messages: bool,
}

impl DirectMessagesChatTopic {
    pub(crate) fn parse(raw: &types::MonoForumDialog, entities: &Entities) -> DirectMessagesChatTopic {
        DirectMessagesChatTopic {
            topic: DirectMessagesTopic::parse(&raw.peer, entities),
            top_message_id: raw.top_message,
            unread_messages_count: raw.unread_count,
            unread_reactions_count: raw.unread_reactions_count,
            last_read_inbox_message_id: raw.read_inbox_max_id,
            last_read_outbox_message_id: raw.read_outbox_max_id,
            unread_mark: raw.unread_mark,
            can_send_unpaid_messages: raw.nopaid_messages_exception,
        }
    }
}
