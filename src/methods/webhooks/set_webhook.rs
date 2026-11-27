use std::sync::Arc;

use crate::Client;
use crate::bot_api::InputFile;
use crate::bot_api::raw::methods;
use crate::bot_api::webhook::{bot_api, check_secret, default_allowed_updates};
use crate::errors::Result;
use crate::methods::accepted;

/// Request built by [`Client::set_webhook`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SetWebhook<'a> {
    client: &'a Client,
    request: methods::SetWebhook,
}

impl Client {
    /// Makes Telegram post the bot's updates to this HTTPS URL instead of keeping them for `getUpdates` (Bot API
    /// mode only: any other client gets `Error::InvalidArgument`). Telegram posts only to ports 443, 80, 88 and
    /// 8443.
    ///
    /// Take the updates with a [`Webhook`](crate::bot_api::Webhook) from [`webhook`](Client::webhook); its
    /// [`register`](crate::bot_api::Webhook::register) calls this with the receiver's secret token already set.
    /// While the webhook is set, [`run`](Client::run) can't poll: it ends with Telegram's 409 error until
    /// [`delete_webhook`](Client::delete_webhook).
    ///
    /// Unless [`allowed_updates`](SetWebhook::allowed_updates) says otherwise, it asks for the kinds of update
    /// vaegram turns into handler calls, every time, so a list set earlier never lingers.
    ///
    /// ```no_run
    /// # async fn f(client: &vaegram::Client) -> vaegram::Result<()> {
    /// client.set_webhook("https://bot.example.com/telegram").secret_token("my_secret_token").await?;
    /// # Ok(()) }
    /// ```
    pub fn set_webhook(&self, url: impl Into<String>) -> SetWebhook<'_> {
        let request = methods::SetWebhook {
            url: url.into(),
            allowed_updates: Some(default_allowed_updates()),
            ..methods::SetWebhook::default()
        };
        SetWebhook { client: self, request }
    }
}

impl SetWebhook<'_> {
    /// The token Telegram sends in the `X-Telegram-Bot-Api-Secret-Token` header of every request: 1 to 256
    /// characters of `A-Z`, `a-z`, `0-9`, `_` and `-`. The [`Webhook`](crate::bot_api::Webhook) that takes the
    /// requests must have the same.
    pub fn secret_token(mut self, token: impl Into<String>) -> Self {
        self.request.secret_token = Some(token.into());
        self
    }

    /// The kinds of update to get, by their Bot API names (`message`, `callback_query`, ...). An empty list means
    /// Telegram's default set. Kinds vaegram doesn't turn into handler calls yet are taken and dropped.
    pub fn allowed_updates<I, S>(mut self, kinds: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.request.allowed_updates = Some(kinds.into_iter().map(Into::into).collect());
        self
    }

    /// Drops the updates waiting to be delivered.
    pub fn drop_pending_updates(mut self) -> Self {
        self.request.drop_pending_updates = Some(true);
        self
    }

    /// The most requests Telegram sends the webhook at once, 1 to 100 (40 unless set); a local Bot API server takes
    /// more. Updates sent at once may come out of order.
    pub fn max_connections(mut self, max: u32) -> Self {
        self.request.max_connections = Some(i64::from(max));
        self
    }

    /// The certificate of a server with a self-signed one, in PEM, so Telegram trusts it. Not needed behind a
    /// certificate from a public authority.
    pub fn certificate(mut self, pem: impl Into<Arc<[u8]>>) -> Self {
        self.request.certificate = Some(InputFile::upload("certificate.pem", pem));
        self
    }

    /// The IP address to send the requests to, instead of the one the URL's host resolves to.
    pub fn ip_address(mut self, ip: impl Into<String>) -> Self {
        self.request.ip_address = Some(ip.into());
        self
    }

    async fn send(self) -> Result<()> {
        let transport = bot_api(self.client)?;
        if let Some(secret) = &self.request.secret_token {
            check_secret(secret)?;
        }
        accepted(transport.api.invoke(&self.request).await?)
    }
}

into_future!(SetWebhook, ());

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;

    #[tokio::test]
    async fn an_mtproto_client_gets_a_clear_error_and_sends_nothing() {
        let (client, mut rx) = Client::recording_raw();
        assert!(matches!(client.set_webhook("https://bot.example.com").await, Err(Error::InvalidArgument(_))));
        assert!(matches!(client.delete_webhook().await, Err(Error::InvalidArgument(_))));
        assert!(matches!(client.get_webhook_info().await, Err(Error::InvalidArgument(_))));
        assert!(matches!(client.webhook("secret"), Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }
}
