use crate::raw::{base, types};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotCommand {
    pub command: String,
    pub description: String,
    pub is_ephemeral: bool,
}

impl BotCommand {
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
