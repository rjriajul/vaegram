#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumTopicCreated {
    pub title: String,
    pub icon_color: i32,
    pub icon_emoji_id: Option<i64>,
    pub is_name_implicit: bool,
}
