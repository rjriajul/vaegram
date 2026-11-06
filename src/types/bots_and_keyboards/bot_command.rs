use crate::raw::{base, types};

/// A command a bot lists in its command menu.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BotCommand {
    /// The command, as the user types it after `/`.
    pub command: String,
    /// What the command does, as shown in the menu.
    pub description: String,
    /// The command's message is seen only by its sender and the bot.
    pub is_ephemeral: bool,
}

impl BotCommand {
    /// Creates a command that is not ephemeral.
    pub fn new(command: impl Into<String>, description: impl Into<String>) -> BotCommand {
        BotCommand { command: command.into(), description: description.into(), is_ephemeral: false }
    }

    /// Makes the command's message ephemeral: only the user who sent the command and the bot see it.
    #[must_use]
    pub fn is_ephemeral(mut self, is_ephemeral: bool) -> Self {
        self.is_ephemeral = is_ephemeral;
        self
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

#[cfg(test)]
mod tests {
    use super::BotCommand;
    use crate::raw::base;

    #[test]
    fn new_is_not_ephemeral_until_set() {
        let base::BotCommand::BotCommand(plain) = BotCommand::new("start", "Starts the bot").write();
        assert_eq!(
            (plain.command.as_str(), plain.description.as_str(), plain.ephemeral),
            ("start", "Starts the bot", false)
        );
        let base::BotCommand::BotCommand(ephemeral) = BotCommand::new("start", "Starts").is_ephemeral(true).write();
        assert!(ephemeral.ephemeral);
    }
}
