use crate::raw::base;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotInfo {
    pub name: String,
    pub short_description: String,
    pub description: String,
}

impl BotInfo {
    pub(crate) fn parse(raw: base::bots::BotInfo) -> BotInfo {
        let base::bots::BotInfo::BotInfo(i) = raw;
        BotInfo { name: i.name, short_description: i.about, description: i.description }
    }
}
