use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::PeerRef;

impl Client {
    /// Revokes the token of a bot you manage and returns the new one. The old token stops working.
    pub async fn replace_managed_bot_token(&self, bot_id: impl Into<PeerRef>) -> Result<String> {
        let bot = self.resolve_user(&bot_id.into()).await?;
        let base::bots::ExportedBotToken::ExportedBotToken(t) =
            self.invoke(&functions::bots::ExportBotToken { bot, revoke: true }).await?;
        Ok(t.token)
    }
}
