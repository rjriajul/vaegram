use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::{base, functions};
use crate::types::{BotCommand, BotCommandScope};

/// Request built by [`Client::set_bot_commands`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SetBotCommands<'a> {
    client: &'a Client,
    commands: Vec<base::BotCommand>,
    scope: BotCommandScope,
    language_code: String,
}

impl Client {
    /// Replaces this bot's command list.
    ///
    /// The list covers the default scope and every language that has no list of its own, unless
    /// [`scope`](SetBotCommands::scope) or [`language_code`](SetBotCommands::language_code) narrow it.
    pub fn set_bot_commands(&self, commands: impl IntoIterator<Item = BotCommand>) -> SetBotCommands<'_> {
        SetBotCommands {
            client: self,
            commands: commands.into_iter().map(BotCommand::write).collect(),
            scope: BotCommandScope::Default,
            language_code: String::new(),
        }
    }
}

impl SetBotCommands<'_> {
    /// Sets the list for these chats or users only.
    pub fn scope(mut self, scope: BotCommandScope) -> Self {
        self.scope = scope;
        self
    }

    /// Sets the list for users with this two-letter language code only.
    pub fn language_code(mut self, language_code: impl Into<String>) -> Self {
        self.language_code = language_code.into();
        self
    }

    async fn send(self) -> Result<()> {
        let request = functions::bots::SetBotCommands {
            scope: self.scope.write(self.client).await?,
            lang_code: self.language_code,
            commands: self.commands,
        };
        accepted(self.client.invoke(&request).await?)
    }
}

into_future!(SetBotCommands, ());
