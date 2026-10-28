use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

#[must_use = "a request does nothing until it is awaited"]
pub struct AnswerPreCheckoutQuery<'a> {
    client: &'a Client,
    query_id: i64,
    error: Option<String>,
}

impl Client {
    /// Telegram cancels the payment unless this is answered within 10 seconds.
    pub fn answer_pre_checkout_query(&self, query_id: i64) -> AnswerPreCheckoutQuery<'_> {
        AnswerPreCheckoutQuery { client: self, query_id, error: None }
    }
}

impl AnswerPreCheckoutQuery<'_> {
    pub fn error(mut self, message: impl Into<String>) -> Self {
        self.error = Some(message.into());
        self
    }

    async fn send(self) -> Result<bool> {
        let request = functions::messages::SetBotPrecheckoutResults {
            success: self.error.is_none(),
            query_id: self.query_id,
            error: self.error,
        };
        self.client.invoke(&request).await
    }
}

into_future!(AnswerPreCheckoutQuery, bool);
