#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumTopic {
    /// Also the `message_thread_id` of the messages in this topic.
    pub id: i32,
    pub title: String,
    pub icon_color: i32,
    pub icon_emoji_id: Option<i64>,
    /// The creator gave no title, so the bot will likely want to rename the topic.
    pub is_name_implicit: bool,
}
