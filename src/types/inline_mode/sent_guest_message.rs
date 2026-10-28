use crate::types::InlineMessageId;

#[derive(Debug, Clone, PartialEq)]
pub struct SentGuestMessage {
    pub inline_message_id: InlineMessageId,
}
