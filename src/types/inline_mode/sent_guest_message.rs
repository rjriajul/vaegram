use crate::types::InlineMessageId;

/// The message sent in answer to a guest query by `GuestMessage::answer`.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SentGuestMessage {
    /// Id of the sent message.
    pub inline_message_id: InlineMessageId,
}
