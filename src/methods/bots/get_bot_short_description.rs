use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};

impl Client {
    pub async fn get_bot_short_description(&self, language_code: &str) -> Result<String> {
        let request = functions::bots::GetBotInfo { bot: None, lang_code: language_code.into() };
        let base::bots::BotInfo::BotInfo(info) = self.invoke(&request).await?;
        Ok(info.about)
    }
}
