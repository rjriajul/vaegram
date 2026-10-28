use crate::types::Photo;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChatShared {
    pub request_id: i32,
    pub chat_id: i64,
    pub title: Option<String>,
    pub username: Option<String>,
    pub photo: Option<Photo>,
}
