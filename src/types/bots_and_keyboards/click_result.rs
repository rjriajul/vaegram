use crate::types::{CallbackAnswer, Message};

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ClickResult {
    Answer(CallbackAnswer),
    Sent(Box<Message>),
    /// A link the user is meant to open: plain url, login url or web app.
    Url(String),
    SwitchInline {
        query: String,
        same_chat: bool,
    },
    Copy(String),
    User(i64),
}
