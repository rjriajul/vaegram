/// Data a Web App sent back to the bot. Only the bot receives `data`. The user's copy carries the button text.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct WebAppData {
    /// Text of the button that opened the Web App.
    pub button_text: String,
    /// The data the Web App sent. `None` in the user's copy of the service message.
    pub data: Option<String>,
}
