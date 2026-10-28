use crate::types::MessageEntity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextQuote {
    pub text: String,
    pub entities: Option<Vec<MessageEntity>>,
    pub position: i32,
    pub is_manual: bool,
}
