use crate::Client;
use crate::bot_api::Webhook;
use crate::errors::Result;

impl Client {
    /// A receiver for the updates Telegram posts to the bot's webhook, checking that each request carries
    /// `secret_token` (Bot API mode only: any other client gets `Error::InvalidArgument`, as does a client built
    /// with `no_updates` or a token Telegram wouldn't take: 1 to 256 characters of `A-Z`, `a-z`, `0-9`, `_` and
    /// `-`). Keep the token out of the code, in the environment or a secret store.
    ///
    /// The handlers run as with polling, once [`Webhook::run`] or `Webhook::serve` (feature `webhook-server`)
    /// takes the updates; [`Client::run`] keeps polling.
    ///
    /// ```no_run
    /// # async fn f(client: vaegram::Client) -> vaegram::Result<()> {
    /// let webhook = client.webhook(std::env::var("WEBHOOK_SECRET").unwrap())?;
    /// webhook.register("https://bot.example.com/telegram").await?;
    /// // Mount `webhook.handle(body, headers)` in your HTTP server, then:
    /// webhook.run().await?;
    /// # Ok(()) }
    /// ```
    pub fn webhook(&self, secret_token: impl Into<String>) -> Result<Webhook> {
        Webhook::new(self, secret_token.into())
    }
}
