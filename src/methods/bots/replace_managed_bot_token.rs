use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};

impl Client {
    pub async fn replace_managed_bot_token(&self, bot_id: i64) -> Result<String> {
        let bot = self.resolve_user(&bot_id.into()).await?;
        let base::bots::ExportedBotToken::ExportedBotToken(t) =
            self.invoke(&functions::bots::ExportBotToken { bot, revoke: true }).await?;
        Ok(t.token)
    }
}
