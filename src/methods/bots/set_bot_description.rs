use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;

/// Request built by [`Client::set_bot_description`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SetBotDescription<'a> {
    client: &'a Client,
    description: String,
    language_code: String,
}

impl Client {
    /// Sets this bot's description for every language that has none of its own.
    ///
    /// [`language_code`](SetBotDescription::language_code) sets it for one language instead.
    pub fn set_bot_description(&self, description: impl Into<String>) -> SetBotDescription<'_> {
        SetBotDescription { client: self, description: description.into(), language_code: String::new() }
    }
}

impl SetBotDescription<'_> {
    /// Sets the description for users with this two-letter language code only.
    pub fn language_code(mut self, language_code: impl Into<String>) -> Self {
        self.language_code = language_code.into();
        self
    }

    async fn send(self) -> Result<()> {
        let request = functions::bots::SetBotInfo {
            bot: None,
            lang_code: self.language_code,
            name: None,
            about: None,
            description: Some(self.description),
        };
        accepted(self.client.invoke(&request).await?)
    }
}

into_future!(SetBotDescription, ());
