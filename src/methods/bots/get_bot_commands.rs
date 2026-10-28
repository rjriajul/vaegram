use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{BotCommand, BotCommandScope};

impl Client {
    pub async fn get_bot_commands(&self, scope: BotCommandScope, language_code: &str) -> Result<Vec<BotCommand>> {
        let request =
            functions::bots::GetBotCommands { scope: scope.write(self).await?, lang_code: language_code.to_owned() };
        Ok(self.invoke(&request).await?.into_iter().map(BotCommand::parse).collect())
    }
}
