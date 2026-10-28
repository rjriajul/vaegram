use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::BotCommandScope;

impl Client {
    pub async fn delete_bot_commands(&self, scope: BotCommandScope, language_code: &str) -> Result<bool> {
        let request =
            functions::bots::ResetBotCommands { scope: scope.write(self).await?, lang_code: language_code.to_owned() };
        self.invoke(&request).await
    }
}
