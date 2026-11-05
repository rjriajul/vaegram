use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::MessageEntity;

/// A message draft saved in a chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    /// Draft text, without formatting.
    pub text: String,
    /// Formatting of the text. `None` if it has none.
    pub entities: Option<Vec<MessageEntity>>,
    /// Id of the message the draft replies to. `None` if it isn't a reply to a message.
    pub reply_to_message_id: Option<i32>,
    /// Forum topic the draft belongs to. `None` outside topics.
    pub topic_id: Option<i32>,
    /// When the draft was last saved, as a Unix timestamp.
    pub date: i32,
}

impl Draft {
    pub(crate) fn parse(raw: &base::DraftMessage, topic_id: Option<i32>, entities: &Entities) -> Option<Draft> {
        let base::DraftMessage::DraftMessage(d) = raw else { return None };
        let reply_to_message_id = match &d.reply_to {
            Some(base::InputReplyTo::InputReplyToMessage(r)) => Some(r.reply_to_msg_id),
            _ => None,
        };
        Some(Draft {
            text: d.message.clone(),
            entities: d.entities.as_deref().and_then(|e| MessageEntity::parse_all(e, entities)),
            reply_to_message_id,
            topic_id,
            date: d.date,
        })
    }
}
