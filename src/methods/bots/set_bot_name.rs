use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;

/// Request built by [`Client::set_bot_name`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SetBotName<'a> {
    client: &'a Client,
    name: String,
    language_code: String,
}

impl Client {
    /// Sets this bot's name for every language that has none of its own.
    ///
    /// [`language_code`](SetBotName::language_code) sets it for one language instead.
    pub fn set_bot_name(&self, name: impl Into<String>) -> SetBotName<'_> {
        SetBotName { client: self, name: name.into(), language_code: String::new() }
    }
}

impl SetBotName<'_> {
    /// Sets the name for users with this two-letter language code only.
    pub fn language_code(mut self, language_code: impl Into<String>) -> Self {
        self.language_code = language_code.into();
        self
    }

    async fn send(self) -> Result<()> {
        let request = functions::bots::SetBotInfo {
            bot: None,
            lang_code: self.language_code,
            name: Some(self.name),
            about: None,
            description: None,
        };
        accepted(self.client.invoke(&request).await?)
    }
}

into_future!(SetBotName, ());
