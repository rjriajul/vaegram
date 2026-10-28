use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::{InlineQueryResult, InlineQueryResultsButton};

#[must_use = "a request does nothing until it is awaited"]
pub struct AnswerInlineQuery<'a> {
    client: &'a Client,
    query_id: i64,
    results: Vec<InlineQueryResult>,
    cache_time: i32,
    personal: bool,
    gallery: bool,
    next_offset: Option<String>,
    button: Option<InlineQueryResultsButton>,
}

impl Client {
    pub fn answer_inline_query<R: Into<InlineQueryResult>>(
        &self,
        query_id: i64,
        results: impl IntoIterator<Item = R>,
    ) -> AnswerInlineQuery<'_> {
        AnswerInlineQuery {
            client: self,
            query_id,
            results: results.into_iter().map(Into::into).collect(),
            cache_time: 300,
            personal: false,
            gallery: false,
            next_offset: None,
            button: None,
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

    pub fn gallery(mut self) -> Self {
        self.gallery = true;
        self
    }

    pub fn next_offset(mut self, offset: impl Into<String>) -> Self {
        self.next_offset = Some(offset.into());
        self
    }

    pub fn button(mut self, button: InlineQueryResultsButton) -> Self {
        self.button = Some(button);
        self
    }

    async fn send(self) -> Result<bool> {
        let mut results = Vec::with_capacity(self.results.len());
        for r in self.results {
            results.push(r.write(self.client).await?);
        }
        let (switch_pm, switch_webview) = match self.button {
            Some(InlineQueryResultsButton::Start { text, start_parameter }) => {
                let pm = types::InlineBotSwitchPM { text, start_param: start_parameter };
                (Some(base::InlineBotSwitchPM::InlineBotSwitchPM(Box::new(pm))), None)
            }
            Some(InlineQueryResultsButton::WebApp { text, url }) => {
                let web_view = types::InlineBotWebView { text, url };
                (None, Some(base::InlineBotWebView::InlineBotWebView(Box::new(web_view))))
            }
            None => (None, None),
        };
        let request = functions::messages::SetInlineBotResults {
            gallery: self.gallery,
            private: self.personal,
            query_id: self.query_id,
            results,
            cache_time: self.cache_time,
            next_offset: self.next_offset,
            switch_pm,
            switch_webview,
        };
        self.client.invoke(&request).await
    }
}

into_future!(AnswerInlineQuery, bool);
