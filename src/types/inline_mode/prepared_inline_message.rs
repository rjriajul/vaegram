#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedInlineMessage {
    pub id: String,
    pub expiration_date: i32,
}
