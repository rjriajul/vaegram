use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{BotPreviewInfo, PeerRef};

impl Client {
    pub async fn get_bot_preview_info(&self, bot_id: impl Into<PeerRef>, lang_code: &str) -> Result<BotPreviewInfo> {
        let bot = self.resolve_user(&bot_id.into()).await?;
        let request = functions::bots::GetPreviewInfo { bot, lang_code: lang_code.into() };
        Ok(BotPreviewInfo::parse(self.invoke(&request).await?))
    }
}
