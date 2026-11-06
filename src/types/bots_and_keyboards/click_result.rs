use crate::types::{CallbackAnswer, Message};

/// What pressing a button on a message did.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ClickResult {
    /// The bot's answer to a callback or game button.
    Answer(CallbackAnswer),
    /// The message a reply keyboard button sent, with the button's label as its text.
    Sent(Box<Message>),
    /// A link the user is meant to open: plain url, login url or web app.
    Url(String),
    /// The button starts an inline query.
    #[non_exhaustive]
    SwitchInline {
        /// The query text to prefill.
        query: String,
        /// Set when the query is for the current chat, not a chat the user picks.
        is_same_chat: bool,
    },
    /// The text the button copies to the clipboard.
    Copy(String),
    /// The id of the user whose profile the button opens.
    User(i64),
}
