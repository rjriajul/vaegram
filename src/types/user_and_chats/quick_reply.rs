use crate::raw::{base, types};

/// A saved quick reply shortcut of a business account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickReply {
    /// Id of the shortcut.
    pub shortcut_id: i32,
    /// The shortcut name.
    pub shortcut: String,
    /// Id of the first message of the reply.
    pub top_message: i32,
    /// Number of messages in the reply.
    pub count: i32,
}

impl QuickReply {
    pub(crate) fn parse(raw: base::QuickReply) -> QuickReply {
        let base::QuickReply::QuickReply(q) = raw;
        let types::QuickReply { shortcut_id, shortcut, top_message, count } = *q;
        QuickReply { shortcut_id, shortcut, top_message, count }
    }
}
