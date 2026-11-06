use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Sets the bot's description for a language. An empty `language_code` sets the default.
    pub async fn set_bot_description(&self, description: &str, language_code: &str) -> Result<()> {
        let request = functions::bots::SetBotInfo {
            bot: None,
            lang_code: language_code.into(),
            name: None,
            about: None,
            description: Some(description.into()),
        };
        self.invoke(&request).await?;
        Ok(())
    }
}
