use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::DirectMessagesTopic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectMessagesChatTopic {
    pub topic: DirectMessagesTopic,
    pub top_message_id: i32,
    pub unread_messages_count: i32,
    pub unread_reactions_count: i32,
    pub last_read_inbox_message_id: i32,
    pub last_read_outbox_message_id: i32,
    pub unread_mark: bool,
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
