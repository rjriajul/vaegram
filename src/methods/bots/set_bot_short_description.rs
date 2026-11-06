use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;

/// Request built by [`Client::set_bot_short_description`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SetBotShortDescription<'a> {
    client: &'a Client,
    short_description: String,
    language_code: String,
}

impl Client {
    /// Sets this bot's short description for every language that has none of its own.
    ///
    /// [`language_code`](SetBotShortDescription::language_code) sets it for one language instead.
    pub fn set_bot_short_description(&self, short_description: impl Into<String>) -> SetBotShortDescription<'_> {
        SetBotShortDescription {
            client: self,
            short_description: short_description.into(),
            language_code: String::new(),
        }
    }
}

impl SetBotShortDescription<'_> {
    /// Sets the short description for users with this two-letter language code only.
    pub fn language_code(mut self, language_code: impl Into<String>) -> Self {
        self.language_code = language_code.into();
        self
    }

    async fn send(self) -> Result<()> {
        let request = functions::bots::SetBotInfo {
            bot: None,
            lang_code: self.language_code,
            name: None,
            about: Some(self.short_description),
            description: None,
        };
        accepted(self.client.invoke(&request).await?)
    }
}

into_future!(SetBotShortDescription, ());
