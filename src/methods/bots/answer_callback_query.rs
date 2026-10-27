use std::future::{Future, IntoFuture};
use std::pin::Pin;

use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

pub struct AnswerCallbackQuery<'a> {
    client: &'a Client,
    query_id: i64,
    text: Option<String>,
    alert: bool,
    url: Option<String>,
    cache_time: i32,
}

impl Client {
    pub fn answer_callback_query(&self, query_id: i64) -> AnswerCallbackQuery<'_> {
        AnswerCallbackQuery { client: self, query_id, text: None, alert: false, url: None, cache_time: 0 }
    }
}

impl AnswerCallbackQuery<'_> {
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    pub fn alert(mut self) -> Self {
        self.alert = true;
        self
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn cache_time(mut self, seconds: i32) -> Self {
        self.cache_time = seconds;
        self
    }
}

impl<'a> IntoFuture for AnswerCallbackQuery<'a> {
    type Output = Result<bool>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let request = functions::messages::SetBotCallbackAnswer {
                alert: self.alert,
                query_id: self.query_id,
                message: self.text,
                url: self.url,
                cache_time: self.cache_time,
            };
            self.client.invoke(&request).await
        })
    }
}
