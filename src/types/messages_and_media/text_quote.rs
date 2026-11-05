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
    /// Offset of the quote in the replied message's text, in UTF-16 code units.
    pub position: i32,
    /// `true` if the sender chose the quote. `false` if Telegram added it.
    pub is_manual: bool,
}
