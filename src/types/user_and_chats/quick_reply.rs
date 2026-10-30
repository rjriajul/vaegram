use crate::raw::{base, types};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickReply {
    pub shortcut_id: i32,
    pub shortcut: String,
    pub top_message: i32,
    pub count: i32,
}

impl QuickReply {
    pub(crate) fn parse(raw: base::QuickReply) -> QuickReply {
        let base::QuickReply::QuickReply(q) = raw;
        let types::QuickReply { shortcut_id, shortcut, top_message, count } = *q;
        QuickReply { shortcut_id, shortcut, top_message, count }
    }
}
