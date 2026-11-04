use crate::types::{MessageEntity, RichText};

#[derive(Debug, Clone, PartialEq)]
pub struct TextQuote {
    pub text: String,
    pub entities: Option<Vec<MessageEntity>>,
    /// The quote as rich text, when it was taken from a rich message.
    pub rich_text: Option<RichText>,
    pub position: i32,
    pub is_manual: bool,
}
