use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{BotAccessSettings, PeerRef};

impl Client {
    /// Returns who can use a bot you manage.
    pub async fn get_managed_bot_access_settings(&self, bot_id: impl Into<PeerRef>) -> Result<BotAccessSettings> {
        let bot = self.resolve_user(&bot_id.into()).await?;
        let raw = self.invoke(&functions::bots::GetAccessSettings { bot }).await?;
        Ok(BotAccessSettings::parse(self, raw))
    }
}
