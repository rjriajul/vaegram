/// Service message data for a new forum topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumTopicCreated {
    /// The topic title.
    pub title: String,
    /// Color of the topic icon.
    pub icon_color: i32,
    /// Id of the custom emoji used as the icon. `None` for the default icon.
    pub icon_emoji_id: Option<i64>,
    /// Whether the topic was created without a title.
    pub is_name_implicit: bool,
}
