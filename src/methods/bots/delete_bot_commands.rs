use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;
use crate::types::BotCommandScope;

impl Client {
    /// Deletes this bot's commands for a scope and language. An empty `language_code` targets the default.
    pub async fn delete_bot_commands(&self, scope: BotCommandScope, language_code: &str) -> Result<()> {
        let request =
            functions::bots::ResetBotCommands { scope: scope.write(self).await?, lang_code: language_code.to_owned() };
        accepted(self.invoke(&request).await?)
    }
}
