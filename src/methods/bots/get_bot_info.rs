use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{BotInfo, PeerRef};

impl Client {
    /// Name and descriptions of a bot you own; an empty `lang_code` gives the default ones.
    pub async fn get_bot_info(&self, bot_id: impl Into<PeerRef>, lang_code: &str) -> Result<BotInfo> {
        let request = functions::bots::GetBotInfo {
            bot: Some(self.resolve_user(&bot_id.into()).await?),
            lang_code: lang_code.into(),
        };
        Ok(BotInfo::parse(self.invoke(&request).await?))
    }
}
