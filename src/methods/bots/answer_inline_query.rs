use std::future::{Future, IntoFuture};
use std::pin::Pin;

use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::{InlineQueryResultArticle, ReplyMarkup};

pub struct AnswerInlineQuery<'a> {
    client: &'a Client,
    query_id: i64,
    results: Vec<InlineQueryResultArticle>,
    cache_time: i32,
    personal: bool,
    next_offset: Option<String>,
}

impl Client {
    pub fn answer_inline_query(
        &self,
        query_id: i64,
        results: impl IntoIterator<Item = InlineQueryResultArticle>,
    ) -> AnswerInlineQuery<'_> {
        AnswerInlineQuery {
            client: self,
            query_id,
            results: results.into_iter().collect(),
            cache_time: 300,
            personal: false,
            next_offset: None,
        }
    }
}

impl AnswerInlineQuery<'_> {
    pub fn cache_time(mut self, seconds: i32) -> Self {
        self.cache_time = seconds;
        self
    }

    pub fn personal(mut self) -> Self {
        self.personal = true;
        self
    }

    pub fn next_offset(mut self, offset: impl Into<String>) -> Self {
        self.next_offset = Some(offset.into());
        self
    }

    async fn send(self) -> Result<bool> {
        let mut results = Vec::with_capacity(self.results.len());
        for r in self.results {
            let (message, entities) = self.client.parse_text(&r.text, r.parse_mode).await?;
            let send_message = types::InputBotInlineMessageText {
                no_webpage: false,
                invert_media: false,
                message,
                entities: (!entities.is_empty()).then_some(entities),
                reply_markup: r.reply_markup.as_ref().map(ReplyMarkup::write),
            };
            results.push(base::InputBotInlineResult::InputBotInlineResult(Box::new(types::InputBotInlineResult {
                id: r.id,
                r#type: "article".into(),
                title: Some(r.title),
                description: r.description,
                url: r.url,
                thumb: None,
                content: None,
                send_message: base::InputBotInlineMessage::InputBotInlineMessageText(Box::new(send_message)),
            })));
        }
        let request = functions::messages::SetInlineBotResults {
            gallery: false,
            private: self.personal,
            query_id: self.query_id,
            results,
            cache_time: self.cache_time,
            next_offset: self.next_offset,
            switch_pm: None,
            switch_webview: None,
        };
        self.client.invoke(&request).await
    }
}

impl<'a> IntoFuture for AnswerInlineQuery<'a> {
    type Output = Result<bool>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}
