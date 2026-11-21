use crate::raw::base;

/// A group of emoji in the emoji picker, such as the smileys or the animals.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct EmojiGroup {
    /// Name of the group.
    pub title: String,
    /// Document id of the custom emoji shown as the group's icon.
    pub icon_emoji_id: i64,
    /// The emoji of the group. Empty for a Premium group, which Telegram doesn't list.
    pub emoticons: Vec<String>,
    /// Telegram marks this one as the greeting group.
    pub is_greeting: bool,
    /// The group holds the emoji only Premium accounts can use.
    pub is_premium: bool,
}

impl EmojiGroup {
    pub(crate) fn parse(raw: &base::EmojiGroup) -> EmojiGroup {
        match raw {
            base::EmojiGroup::EmojiGroup(g) => EmojiGroup {
                title: g.title.clone(),
                icon_emoji_id: g.icon_emoji_id,
                emoticons: g.emoticons.clone(),
                is_greeting: false,
                is_premium: false,
            },
            base::EmojiGroup::EmojiGroupGreeting(g) => EmojiGroup {
                title: g.title.clone(),
                icon_emoji_id: g.icon_emoji_id,
                emoticons: g.emoticons.clone(),
                is_greeting: true,
                is_premium: false,
            },
            base::EmojiGroup::EmojiGroupPremium(g) => EmojiGroup {
                title: g.title.clone(),
                icon_emoji_id: g.icon_emoji_id,
                emoticons: Vec::new(),
                is_greeting: false,
                is_premium: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EmojiGroup;
    use crate::raw::{base, types};

    #[test]
    fn reads_each_kind_of_group() {
        let plain = types::EmojiGroup { title: "Smileys".into(), icon_emoji_id: 5, emoticons: vec!["😀".into()] };
        let plain = EmojiGroup::parse(&base::EmojiGroup::EmojiGroup(Box::new(plain)));
        assert_eq!((plain.title.as_str(), plain.icon_emoji_id), ("Smileys", 5));
        assert_eq!(plain.emoticons, ["😀"]);
        assert!(!plain.is_greeting && !plain.is_premium);

        let hello =
            types::EmojiGroupGreeting { title: "Hello".into(), icon_emoji_id: 6, emoticons: vec!["👋".into()] };
        let hello = EmojiGroup::parse(&base::EmojiGroup::EmojiGroupGreeting(Box::new(hello)));
        assert!(hello.is_greeting && !hello.is_premium);
        assert_eq!(hello.emoticons, ["👋"]);

        let premium = types::EmojiGroupPremium { title: "Premium".into(), icon_emoji_id: 7 };
        let premium = EmojiGroup::parse(&base::EmojiGroup::EmojiGroupPremium(Box::new(premium)));
        assert!(premium.is_premium && !premium.is_greeting && premium.emoticons.is_empty());
    }
}
