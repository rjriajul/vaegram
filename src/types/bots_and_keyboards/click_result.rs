use crate::types::{CallbackAnswer, Message};

/// What pressing a button on a message did.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ClickResult {
    /// A callback or game button was pressed and the bot answered.
    Answer(CallbackAnswer),
    /// A reply keyboard button sent its label as a message. This is the sent message.
    Sent(Box<Message>),
    /// A link the user is meant to open: plain url, login url or web app.
    Url(String),
    /// The button starts an inline query.
    SwitchInline {
        /// The query text to prefill.
        query: String,
        /// The query is for the current chat, not a chat the user picks.
        same_chat: bool,
    },
    /// The text the button copies to the clipboard.
    Copy(String),
    /// The id of the user whose profile the button opens.
    User(i64),
}
