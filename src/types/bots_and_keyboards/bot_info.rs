use crate::raw::base;

/// A bot's name and descriptions in one language.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BotInfo {
    /// The bot's name.
    pub name: String,
    /// The bot's short description (its about text).
    pub short_description: String,
    /// The text shown in an empty chat with the bot.
    pub description: String,
}

impl BotInfo {
    pub(crate) fn parse(raw: base::bots::BotInfo) -> BotInfo {
        let base::bots::BotInfo::BotInfo(i) = raw;
        BotInfo { name: i.name, short_description: i.about, description: i.description }
    }
}
