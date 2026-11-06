use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;

/// Request built by [`Client::answer_pre_checkout_query`].
#[must_use = "a request does nothing until it is awaited"]
pub struct AnswerPreCheckoutQuery<'a> {
    client: &'a Client,
    query_id: i64,
    error: Option<String>,
}

impl Client {
    /// Answers a pre-checkout query. The checkout is approved unless [`error`](AnswerPreCheckoutQuery::error) is set.
    pub fn answer_pre_checkout_query(&self, query_id: i64) -> AnswerPreCheckoutQuery<'_> {
        AnswerPreCheckoutQuery { client: self, query_id, error: None }
    }
}

impl AnswerPreCheckoutQuery<'_> {
    /// Rejects the checkout and shows `message` to the user. Without it the checkout is approved.
    pub fn error(mut self, message: impl Into<String>) -> Self {
        self.error = Some(message.into());
        self
    }

    async fn send(self) -> Result<()> {
        let request = functions::messages::SetBotPrecheckoutResults {
            success: self.error.is_none(),
            query_id: self.query_id,
            error: self.error,
        };
        accepted(self.client.invoke(&request).await?)
    }
}

into_future!(AnswerPreCheckoutQuery, ());
