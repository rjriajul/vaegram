use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;
use crate::types::BotCommandScope;

/// Request built by [`Client::delete_bot_commands`].
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteBotCommands<'a> {
    client: &'a Client,
    scope: BotCommandScope,
    language_code: String,
}

impl Client {
    /// Deletes this bot's command list for the default scope and language.
    ///
    /// [`scope`](DeleteBotCommands::scope) and [`language_code`](DeleteBotCommands::language_code) pick another
    /// list. Users the deleted list covered see the next wider one.
    pub fn delete_bot_commands(&self) -> DeleteBotCommands<'_> {
        DeleteBotCommands { client: self, scope: BotCommandScope::Default, language_code: String::new() }
    }
}

impl DeleteBotCommands<'_> {
    /// Deletes the list set for these chats or users.
    pub fn scope(mut self, scope: BotCommandScope) -> Self {
        self.scope = scope;
        self
    }

    /// Deletes the list set for users with this two-letter language code.
    pub fn language_code(mut self, language_code: impl Into<String>) -> Self {
        self.language_code = language_code.into();
        self
    }

    async fn send(self) -> Result<()> {
        let request = functions::bots::ResetBotCommands {
            scope: self.scope.write(self.client).await?,
            lang_code: self.language_code,
        };
        accepted(self.client.invoke(&request).await?)
    }
}

into_future!(DeleteBotCommands, ());
