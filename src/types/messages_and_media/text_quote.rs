use crate::types::{MessageEntity, RichText};

/// A quoted part of a message that another message replies to.
#[derive(Debug, Clone, PartialEq)]
pub struct TextQuote {
    /// The quoted text.
    pub text: String,
    /// Formatting entities in `text`. `None` if there are none.
    pub entities: Option<Vec<MessageEntity>>,
    /// The quote as rich text, when it was taken from a rich message.
    pub rich_text: Option<RichText>,
    /// Offset of the quote in the original message's text.
    pub position: i32,
    /// `true` if the sender picked the quoted part by hand instead of Telegram quoting the whole message.
    pub is_manual: bool,
}
