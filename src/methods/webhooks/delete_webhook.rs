use crate::Client;
use crate::bot_api::raw::methods;
use crate::bot_api::webhook::bot_api;
use crate::errors::Result;
use crate::methods::accepted;

/// Request built by [`Client::delete_webhook`].
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteWebhook<'a> {
    client: &'a Client,
    drop_pending_updates: bool,
}

impl Client {
    /// Removes the bot's webhook, so Telegram keeps updates for `getUpdates` again and [`run`](Client::run) can
    /// poll (Bot API mode only: any other client gets `Error::InvalidArgument`). Telegram keeps the updates that
    /// waited for delivery, unless [`drop_pending_updates`](DeleteWebhook::drop_pending_updates) is set.
    ///
    /// ```no_run
    /// # async fn f(client: &vaegram::Client) -> vaegram::Result<()> {
    /// client.delete_webhook().await?;
    /// client.run().await?;
    /// # Ok(()) }
    /// ```
    pub fn delete_webhook(&self) -> DeleteWebhook<'_> {
        DeleteWebhook { client: self, drop_pending_updates: false }
    }
}

impl DeleteWebhook<'_> {
    /// Drops the updates waiting to be delivered too.
    pub fn drop_pending_updates(mut self) -> Self {
        self.drop_pending_updates = true;
        self
    }

    async fn send(self) -> Result<()> {
        let transport = bot_api(self.client)?;
        let request = methods::DeleteWebhook { drop_pending_updates: self.drop_pending_updates.then_some(true) };
        accepted(transport.api.invoke(&request).await?)
    }
}

into_future!(DeleteWebhook, ());
