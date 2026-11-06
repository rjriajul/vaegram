use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};

/// Request built by [`Client::get_bot_name`].
#[must_use = "a request does nothing until it is awaited"]
pub struct GetBotName<'a> {
    client: &'a Client,
    language_code: String,
}

impl Client {
    /// Returns this bot's default name, the one users of a language without its own see.
    ///
    /// [`language_code`](GetBotName::language_code) returns the one for a language instead.
    pub fn get_bot_name(&self) -> GetBotName<'_> {
        GetBotName { client: self, language_code: String::new() }
    }
}

impl GetBotName<'_> {
    /// Returns the name set for users with this two-letter language code.
    pub fn language_code(mut self, language_code: impl Into<String>) -> Self {
        self.language_code = language_code.into();
        self
    }

    async fn send(self) -> Result<String> {
        let request = functions::bots::GetBotInfo { bot: None, lang_code: self.language_code };
        let base::bots::BotInfo::BotInfo(info) = self.client.invoke(&request).await?;
        Ok(info.name)
    }
}

into_future!(GetBotName, String);
