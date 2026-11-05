use crate::raw::{base, types};

/// A command a bot lists in its command menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotCommand {
    /// The command, as the user types it after `/`.
    pub command: String,
    /// What the command does, as shown in the menu.
    pub description: String,
    /// Sent to Telegram as the command's `ephemeral` flag.
    pub is_ephemeral: bool,
}

impl BotCommand {
    /// Creates a command that is not ephemeral.
    pub fn new(command: impl Into<String>, description: impl Into<String>) -> BotCommand {
        BotCommand { command: command.into(), description: description.into(), is_ephemeral: false }
    }

    pub(crate) fn parse(raw: base::BotCommand) -> BotCommand {
        let base::BotCommand::BotCommand(c) = raw;
        BotCommand { command: c.command, description: c.description, is_ephemeral: c.ephemeral }
    }

    pub(crate) fn write(self) -> base::BotCommand {
        base::BotCommand::BotCommand(Box::new(types::BotCommand {
            ephemeral: self.is_ephemeral,
            command: self.command,
            description: self.description,
        }))
    }
}
