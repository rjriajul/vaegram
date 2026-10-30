use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::MessageEntity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub text: String,
    pub entities: Option<Vec<MessageEntity>>,
    pub reply_to_message_id: Option<i32>,
    pub topic_id: Option<i32>,
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
