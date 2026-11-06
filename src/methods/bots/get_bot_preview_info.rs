use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{BotPreviewInfo, PeerRef};

/// Request built by [`Client::get_bot_preview_info`].
#[must_use = "a request does nothing until it is awaited"]
pub struct GetBotPreviewInfo<'a> {
    client: &'a Client,
    bot_id: PeerRef,
    language_code: String,
}

impl Client {
    /// Returns your bot's default preview media, shown in languages without previews of their own, and the
    /// languages that have previews.
    ///
    /// [`language_code`](GetBotPreviewInfo::language_code) returns one language's previews instead. For a bot you
    /// don't own, Telegram returns only the previews for the current account's language and no `lang_codes`.
    pub fn get_bot_preview_info(&self, bot_id: impl Into<PeerRef>) -> GetBotPreviewInfo<'_> {
        GetBotPreviewInfo { client: self, bot_id: bot_id.into(), language_code: String::new() }
    }
}

impl GetBotPreviewInfo<'_> {
    /// Returns the previews for users with this two-letter language code.
    pub fn language_code(mut self, language_code: impl Into<String>) -> Self {
        self.language_code = language_code.into();
        self
    }

    async fn send(self) -> Result<BotPreviewInfo> {
        let bot = self.client.resolve_user(&self.bot_id).await?;
        let request = functions::bots::GetPreviewInfo { bot, lang_code: self.language_code };
        Ok(BotPreviewInfo::parse(self.client, self.client.invoke(&request).await?))
    }
}

into_future!(GetBotPreviewInfo, BotPreviewInfo);
