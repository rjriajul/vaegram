use crate::Client;
use crate::bot_api::WebhookInfo;
use crate::bot_api::raw::methods;
use crate::bot_api::webhook::bot_api;
use crate::errors::Result;

impl Client {
    /// Where the bot's webhook stands: its URL (empty when none is set), how many updates wait, and the last
    /// delivery error (Bot API mode only: any other client gets `Error::InvalidArgument`).
    ///
    /// ```no_run
    /// # async fn f(client: &vaegram::Client) -> vaegram::Result<()> {
    /// let info = client.get_webhook_info().await?;
    /// if let Some(error) = &info.last_error_message {
    ///     eprintln!("{} updates wait; last error: {error}", info.pending_update_count);
    /// }
    /// # Ok(()) }
    /// ```
    pub async fn get_webhook_info(&self) -> Result<WebhookInfo> {
        let transport = bot_api(self)?;
        let info = transport.api.invoke(&methods::GetWebhookInfo {}).await?;
        Ok(WebhookInfo::from_raw(info))
    }
}
