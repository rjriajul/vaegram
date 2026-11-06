use crate::raw::base;

/// The bot's reply to a callback button press.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct CallbackAnswer {
    /// Message to show the user. `None` if the bot sent none.
    pub text: Option<String>,
    /// Shows the text as an alert instead of a brief notice.
    pub is_alert: bool,
    /// A link for the client to open. `None` if the bot sent none.
    pub url: Option<String>,
    /// Seconds the client may reuse this answer without asking the bot again.
    pub cache_time: i32,
}

impl CallbackAnswer {
    pub(crate) fn parse(raw: base::messages::BotCallbackAnswer) -> CallbackAnswer {
        let base::messages::BotCallbackAnswer::BotCallbackAnswer(a) = raw;
        CallbackAnswer { text: a.message, is_alert: a.alert, url: a.url, cache_time: a.cache_time }
    }
}
