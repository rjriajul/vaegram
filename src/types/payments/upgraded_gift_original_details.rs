use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::messages_and_media::text_with_entities;
use crate::types::{Chat, MessageEntity};

/// Who sent a gift before it was upgraded, to whom, and with what message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradedGiftOriginalDetails {
    pub sender: Option<Chat>,
    pub receiver: Chat,
    pub date: i32,
    pub text: Option<String>,
    pub entities: Option<Vec<MessageEntity>>,
}

impl UpgradedGiftOriginalDetails {
    pub(crate) fn parse(raw: &types::StarGiftAttributeOriginalDetails, entities: &Entities) -> Self {
        let (text, text_entities) = match raw.message.as_ref().map(|m| text_with_entities(m, entities)) {
            Some((text, entities)) => (Some(text), entities),
            None => (None, None),
        };
        UpgradedGiftOriginalDetails {
            sender: raw.sender_id.as_ref().map(|p| Chat::parse(p, entities, true)),
            receiver: Chat::parse(&raw.recipient_id, entities, true),
            date: raw.date,
            text,
            entities: text_entities,
        }
    }
}
