use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;

/// Request built by [`Client::answer_widget_query`].
#[must_use = "a request does nothing until it is awaited"]
pub struct AnswerWidgetQuery<'a> {
    client: &'a Client,
    query_id: i64,
    data: Option<String>,
}

impl Client {
    /// Answers a widget query within 15 seconds of receiving it.
    ///
    /// The answer reaches only the asking widget and does not change its data. To change widget data, use
    /// [`Client::edit_message_widget_data`] or [`Client::edit_shared_widget_data`].
    pub fn answer_widget_query(&self, query_id: i64) -> AnswerWidgetQuery<'_> {
        AnswerWidgetQuery { client: self, query_id, data: None }
    }
}

impl AnswerWidgetQuery<'_> {
    /// Data returned to the asking widget.
    pub fn data(mut self, data: impl Into<String>) -> Self {
        self.data = Some(data.into());
        self
    }

    async fn send(self) -> Result<()> {
        let request = functions::messages::SetBotWidgetQueryAnswer { query_id: self.query_id, result: self.data };
        accepted(self.client.invoke(&request).await?)
    }
}

into_future!(AnswerWidgetQuery, ());

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, functions};

    #[tokio::test]
    async fn sends_widget_answer_data() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes())]);
        client.answer_widget_query(42).data("{\"saved\":true}").await.unwrap();
        let request =
            functions::messages::SetBotWidgetQueryAnswer { query_id: 42, result: Some("{\"saved\":true}".into()) };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
    }
}
