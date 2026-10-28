use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn set_bot_description(&self, description: &str, language_code: &str) -> Result<bool> {
        let request = functions::bots::SetBotInfo {
            bot: None,
            lang_code: language_code.into(),
            name: None,
            about: None,
            description: Some(description.into()),
        };
        self.invoke(&request).await
    }
}
