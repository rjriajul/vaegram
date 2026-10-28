use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{BotCommand, BotCommandScope};

impl Client {
    /// An empty `language_code` covers users with no dedicated list.
    pub async fn set_bot_commands(
        &self,
        commands: impl IntoIterator<Item = BotCommand>,
        scope: BotCommandScope,
        language_code: &str,
    ) -> Result<bool> {
        let request = functions::bots::SetBotCommands {
            scope: scope.write(self).await?,
            lang_code: language_code.to_owned(),
            commands: commands.into_iter().map(BotCommand::write).collect(),
        };
        self.invoke(&request).await
    }
}
