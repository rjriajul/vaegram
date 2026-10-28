#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineQueryResultsButton {
    /// Opens a private chat with the bot, sending `/start start_parameter`.
    Start {
        text: String,
        start_parameter: String,
    },
    WebApp {
        text: String,
        url: String,
    },
}

impl InlineQueryResultsButton {
    pub fn start(text: impl Into<String>, start_parameter: impl Into<String>) -> Self {
        InlineQueryResultsButton::Start { text: text.into(), start_parameter: start_parameter.into() }
    }

    pub fn web_app(text: impl Into<String>, url: impl Into<String>) -> Self {
        InlineQueryResultsButton::WebApp { text: text.into(), url: url.into() }
    }
}
