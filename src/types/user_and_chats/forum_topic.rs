use crate::raw::base;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumTopic {
    /// Also the `message_thread_id` of the messages in this topic.
    pub id: i32,
    pub title: String,
    pub icon_color: i32,
    pub icon_emoji_id: Option<i64>,
    /// The creator gave no title, so the bot will likely want to rename the topic.
    pub is_name_implicit: bool,
    pub date: i32,
    pub top_message_id: i32,
    pub unread_messages_count: i32,
    pub unread_mentions_count: i32,
    pub unread_reactions_count: i32,
    pub is_closed: bool,
    pub is_pinned: bool,
    pub is_hidden: bool,
}

impl ForumTopic {
    pub(crate) fn parse(raw: base::ForumTopic) -> Option<ForumTopic> {
        let base::ForumTopic::ForumTopic(t) = raw else { return None };
        let t = *t;
        Some(ForumTopic {
            id: t.id,
            title: t.title,
            icon_color: t.icon_color,
            icon_emoji_id: t.icon_emoji_id,
            is_name_implicit: t.title_missing,
            date: t.date,
            top_message_id: t.top_message,
            unread_messages_count: t.unread_count,
            unread_mentions_count: t.unread_mentions_count,
            unread_reactions_count: t.unread_reactions_count,
            is_closed: t.closed,
            is_pinned: t.pinned,
            is_hidden: t.hidden,
        })
    }
}
