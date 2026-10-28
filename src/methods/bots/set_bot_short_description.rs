use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn set_bot_short_description(&self, short_description: &str, language_code: &str) -> Result<bool> {
        let request = functions::bots::SetBotInfo {
            bot: None,
            lang_code: language_code.into(),
            name: None,
            about: Some(short_description.into()),
            description: None,
        };
        self.invoke(&request).await
    }
}
