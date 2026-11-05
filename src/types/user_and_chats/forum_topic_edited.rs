/// Service message data for an edited forum topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumTopicEdited {
    /// The new title. `None` if it didn't change.
    pub title: Option<String>,
    /// Custom emoji id of the new icon. `None` if it didn't change, `Some(0)` if the icon was removed.
    pub icon_emoji_id: Option<i64>,
}
