/// A button shown above the results of an inline query.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InlineQueryResultsButton {
    /// Opens a private chat with the bot, sending `/start start_parameter`.
    Start {
        /// Button label.
        text: String,
        /// Text sent after `/start`.
        start_parameter: String,
    },
    /// Opens the mini app at `url`.
    WebApp {
        /// Button label.
        text: String,
        /// URL of the mini app.
        url: String,
    },
}

impl InlineQueryResultsButton {
    /// Creates a button that opens a private chat with the bot and sends `/start start_parameter`.
    pub fn start(text: impl Into<String>, start_parameter: impl Into<String>) -> Self {
        InlineQueryResultsButton::Start { text: text.into(), start_parameter: start_parameter.into() }
    }

    /// Creates a button that opens the mini app at `url`.
    pub fn web_app(text: impl Into<String>, url: impl Into<String>) -> Self {
        InlineQueryResultsButton::WebApp { text: text.into(), url: url.into() }
    }
}
