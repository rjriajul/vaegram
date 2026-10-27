use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::InlineQueryResultArticle;

#[must_use = "a request does nothing until it is awaited"]
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
            results.push(r.write(self.client).await?);
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

into_future!(AnswerInlineQuery, bool);
