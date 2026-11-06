use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::ShippingOption;

/// Request built by [`Client::answer_shipping_query`].
#[must_use = "a request does nothing until it is awaited"]
pub struct AnswerShippingQuery<'a> {
    client: &'a Client,
    query_id: i64,
    shipping_options: Option<Vec<ShippingOption>>,
    error: Option<String>,
}

impl Client {
    /// Answers a shipping query from a payment with a flexible price.
    ///
    /// Set either [`shipping_options`](AnswerShippingQuery::shipping_options) or
    /// [`error`](AnswerShippingQuery::error).
    pub fn answer_shipping_query(&self, query_id: i64) -> AnswerShippingQuery<'_> {
        AnswerShippingQuery { client: self, query_id, shipping_options: None, error: None }
    }
}

impl AnswerShippingQuery<'_> {
    /// Offers these shipping options to the user.
    pub fn shipping_options(mut self, options: impl IntoIterator<Item = ShippingOption>) -> Self {
        self.shipping_options = Some(options.into_iter().collect());
        self
    }

    /// Tells the user that delivery isn't possible and shows `message`.
    pub fn error(mut self, message: impl Into<String>) -> Self {
        self.error = Some(message.into());
        self
    }

    async fn send(self) -> Result<()> {
        let request = functions::messages::SetBotShippingResults {
            query_id: self.query_id,
            error: self.error,
            shipping_options: self.shipping_options.map(|o| o.into_iter().map(ShippingOption::write).collect()),
        };
        self.client.invoke(&request).await?;
        Ok(())
    }
}

into_future!(AnswerShippingQuery, ());
