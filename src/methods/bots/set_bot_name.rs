use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn set_bot_name(&self, name: &str, language_code: &str) -> Result<bool> {
        let request = functions::bots::SetBotInfo {
            bot: None,
            lang_code: language_code.into(),
            name: Some(name.into()),
            about: None,
            description: None,
        };
        self.invoke(&request).await
    }
}
