#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumTopicEdited {
    pub title: Option<String>,
    /// The new icon, if it changed; `Some(0)` means the icon was removed.
    pub icon_emoji_id: Option<i64>,
}
