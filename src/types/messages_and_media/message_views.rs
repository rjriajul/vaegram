use crate::raw::base;

/// View, forward and reply counters of a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct MessageViews {
    /// Number of views. `None` if Telegram sent no count.
    pub views: Option<i32>,
    /// Number of forwards. `None` if Telegram sent no count.
    pub forwards: Option<i32>,
    /// Number of replies in the message's comment thread. `None` if the message has no comment thread.
    pub replies: Option<i32>,
}

impl MessageViews {
    pub(crate) fn parse(raw: &base::MessageViews) -> MessageViews {
        let base::MessageViews::MessageViews(v) = raw;
        let replies = v.replies.as_ref().map(|base::MessageReplies::MessageReplies(r)| r.replies);
        MessageViews { views: v.views, forwards: v.forwards, replies }
    }
}
