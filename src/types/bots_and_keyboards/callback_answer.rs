use crate::raw::base;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackAnswer {
    pub text: Option<String>,
    pub alert: bool,
    pub url: Option<String>,
    pub cache_time: i32,
}

impl CallbackAnswer {
    pub(crate) fn parse(raw: base::messages::BotCallbackAnswer) -> CallbackAnswer {
        let base::messages::BotCallbackAnswer::BotCallbackAnswer(a) = raw;
        CallbackAnswer { text: a.message, alert: a.alert, url: a.url, cache_time: a.cache_time }
    }
}
