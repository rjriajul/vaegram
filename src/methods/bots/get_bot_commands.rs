use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{BotCommand, BotCommandScope};

/// Request built by [`Client::get_bot_commands`].
#[must_use = "a request does nothing until it is awaited"]
pub struct GetBotCommands<'a> {
    client: &'a Client,
    scope: BotCommandScope,
    language_code: String,
}

impl Client {
    /// Returns this bot's command list for the default scope and language.
    ///
    /// [`scope`](GetBotCommands::scope) and [`language_code`](GetBotCommands::language_code) pick another list.
    pub fn get_bot_commands(&self) -> GetBotCommands<'_> {
        GetBotCommands { client: self, scope: BotCommandScope::Default, language_code: String::new() }
    }
}

impl GetBotCommands<'_> {
    /// Returns the list set for these chats or users.
    pub fn scope(mut self, scope: BotCommandScope) -> Self {
        self.scope = scope;
        self
    }

    /// Returns the list set for users with this two-letter language code.
    pub fn language_code(mut self, language_code: impl Into<String>) -> Self {
        self.language_code = language_code.into();
        self
    }

    async fn send(self) -> Result<Vec<BotCommand>> {
        let request = functions::bots::GetBotCommands {
            scope: self.scope.write(self.client).await?,
            lang_code: self.language_code,
        };
        Ok(self.client.invoke(&request).await?.into_iter().map(BotCommand::parse).collect())
    }
}

into_future!(GetBotCommands, Vec<BotCommand>);
