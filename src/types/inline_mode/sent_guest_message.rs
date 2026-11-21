use crate::types::InlineMessageId;

/// The message sent in answer to a guest query by `GuestMessage::answer`, `GuestMessage::answer_text` or
/// `GuestMessage::answer_rich`.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SentGuestMessage {
    /// Id of the sent message.
    pub inline_message_id: InlineMessageId,
}
