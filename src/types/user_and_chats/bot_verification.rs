use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::{FormattedText, User};

/// A verification mark given by a bot that verifies users and chats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotVerification {
    pub bot: User,
    /// The icon shown next to the name.
    pub custom_emoji_id: i64,
    pub description: FormattedText,
}

impl BotVerification {
    pub(crate) fn parse(raw: base::BotVerification, entities: &Entities) -> BotVerification {
        let base::BotVerification::BotVerification(v) = raw;
        let v = *v;
        BotVerification {
            bot: User::parse(v.bot_id, entities),
            custom_emoji_id: v.icon,
            description: FormattedText::parse(v.description),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BotVerification;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};

    #[test]
    fn reads_bot_icon_and_text() {
        let text = types::TextWithEntities { text: "Official".into(), entities: Vec::new() };
        let raw = types::BotVerification {
            bot_id: 9,
            icon: 42,
            description: base::TextWithEntities::TextWithEntities(text.into()),
        };
        let parsed = BotVerification::parse(base::BotVerification::BotVerification(raw.into()), &Entities::default());
        assert_eq!((parsed.bot.id, parsed.custom_emoji_id, parsed.description.text.as_str()), (9, 42, "Official"));
    }
}
