use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::messages_and_media::text_with_entities;
use crate::types::{Chat, MessageEntity};

/// Who sent a gift before it was upgraded, to whom, and with what message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradedGiftOriginalDetails {
    /// Who sent the gift. `None` if the sender was hidden.
    pub sender: Option<Chat>,
    /// Who received the gift.
    pub receiver: Chat,
    /// Unix time the gift was sent.
    pub date: i32,
    /// The text the sender attached. `None` if there was none.
    pub text: Option<String>,
    /// Formatting entities of `text`. `None` when there is no text or it has no formatting.
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
