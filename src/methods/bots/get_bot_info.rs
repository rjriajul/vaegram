use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{BotInfo, PeerRef};

/// Request built by [`Client::get_bot_info`].
#[must_use = "a request does nothing until it is awaited"]
pub struct GetBotInfo<'a> {
    client: &'a Client,
    bot_id: PeerRef,
    language_code: String,
}

impl Client {
    /// Returns the default name and descriptions of a bot you own, the ones users of a language without their own
    /// see.
    ///
    /// [`language_code`](GetBotInfo::language_code) returns the ones for a language instead.
    pub fn get_bot_info(&self, bot_id: impl Into<PeerRef>) -> GetBotInfo<'_> {
        GetBotInfo { client: self, bot_id: bot_id.into(), language_code: String::new() }
    }
}

impl GetBotInfo<'_> {
    /// Returns the name and descriptions set for users with this two-letter language code.
    pub fn language_code(mut self, language_code: impl Into<String>) -> Self {
        self.language_code = language_code.into();
        self
    }

    async fn send(self) -> Result<BotInfo> {
        let bot = Some(self.client.resolve_user(&self.bot_id).await?);
        let request = functions::bots::GetBotInfo { bot, lang_code: self.language_code };
        Ok(BotInfo::parse(self.client.invoke(&request).await?))
    }
}

into_future!(GetBotInfo, BotInfo);
