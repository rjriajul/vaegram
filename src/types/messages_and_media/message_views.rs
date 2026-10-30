use crate::raw::base;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MessageViews {
    pub views: Option<i32>,
    pub forwards: Option<i32>,
    pub replies: Option<i32>,
}

impl MessageViews {
    pub(crate) fn parse(raw: &base::MessageViews) -> MessageViews {
        let base::MessageViews::MessageViews(v) = raw;
        let replies = v.replies.as_ref().map(|base::MessageReplies::MessageReplies(r)| r.replies);
        MessageViews { views: v.views, forwards: v.forwards, replies }
    }
}
