use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::BotAccessSettings;

impl Client {
    pub async fn get_managed_bot_access_settings(&self, bot_id: i64) -> Result<BotAccessSettings> {
        let bot = self.resolve_user(&bot_id.into()).await?;
        let raw = self.invoke(&functions::bots::GetAccessSettings { bot }).await?;
        Ok(BotAccessSettings::parse(self, raw))
    }
}
