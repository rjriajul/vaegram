use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

/// Request built by [`Client::answer_callback_query`]. Awaiting it returns whether Telegram accepted the answer.
#[must_use = "a request does nothing until it is awaited"]
pub struct AnswerCallbackQuery<'a> {
    client: &'a Client,
    query_id: i64,
    text: Option<String>,
    alert: bool,
    url: Option<String>,
    cache_time: i32,
}

impl Client {
    /// Answers a callback query from an inline keyboard button.
    ///
    /// Awaiting the request returns whether Telegram accepted the answer.
    ///
    /// ```no_run
    /// # async fn f(client: &vaegram::Client) -> vaegram::Result<()> {
    /// # let query_id = 0;
    /// client.answer_callback_query(query_id).text("Saved").await?;
    /// # Ok(()) }
    /// ```
    pub fn answer_callback_query(&self, query_id: i64) -> AnswerCallbackQuery<'_> {
        AnswerCallbackQuery { client: self, query_id, text: None, alert: false, url: None, cache_time: 0 }
    }
}

impl AnswerCallbackQuery<'_> {
    /// Text the client shows to the user, as a toast or as an alert.
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Shows the text as a modal alert instead of a toast.
    pub fn alert(mut self) -> Self {
        self.alert = true;
        self
    }

    /// Makes the client open this URL.
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// How long, in seconds, the client may cache this answer for the same button. Defaults to 0.
    pub fn cache_time(mut self, seconds: i32) -> Self {
        self.cache_time = seconds;
        self
    }

    async fn send(self) -> Result<bool> {
        let request = functions::messages::SetBotCallbackAnswer {
            alert: self.alert,
            query_id: self.query_id,
            message: self.text,
            url: self.url,
            cache_time: self.cache_time,
        };
        self.client.invoke(&request).await
    }
}

into_future!(AnswerCallbackQuery, bool);
