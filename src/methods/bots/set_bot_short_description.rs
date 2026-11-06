use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;

impl Client {
    /// Sets the bot's short description for a language. An empty `language_code` sets the default.
    pub async fn set_bot_short_description(&self, short_description: &str, language_code: &str) -> Result<()> {
        let request = functions::bots::SetBotInfo {
            bot: None,
            lang_code: language_code.into(),
            name: None,
            about: Some(short_description.into()),
            description: None,
        };
        accepted(self.invoke(&request).await?)
    }
}
