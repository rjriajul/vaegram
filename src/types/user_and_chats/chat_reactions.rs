use crate::raw::base;
use crate::types::ReactionType;

/// The reactions members may put on messages; the default allows none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatReactions {
    /// Every emoji reaction is allowed.
    pub all_are_enabled: bool,
    /// With `all_are_enabled`, custom emoji are allowed too.
    pub allow_custom_emoji: bool,
    /// The allowed reactions. Empty when `all_are_enabled` is set.
    pub reactions: Vec<ReactionType>,
}

impl ChatReactions {
    pub(crate) fn parse(raw: &base::ChatReactions) -> ChatReactions {
        match raw {
            base::ChatReactions::ChatReactionsNone => ChatReactions::default(),
            base::ChatReactions::ChatReactionsAll(r) => {
                ChatReactions { all_are_enabled: true, allow_custom_emoji: r.allow_custom, reactions: Vec::new() }
            }
            base::ChatReactions::ChatReactionsSome(r) => ChatReactions {
                all_are_enabled: false,
                allow_custom_emoji: false,
                reactions: r.reactions.iter().filter_map(ReactionType::parse).collect(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ChatReactions;
    use crate::raw::{base, types};
    use crate::types::ReactionType;

    #[test]
    fn all_some_and_none_reactions() {
        let all = base::ChatReactions::ChatReactionsAll(types::ChatReactionsAll { allow_custom: true });
        let all = ChatReactions::parse(&all);
        assert_eq!((all.all_are_enabled, all.allow_custom_emoji, all.reactions.len()), (true, true, 0));

        let like = base::Reaction::ReactionEmoji(Box::new(types::ReactionEmoji { emoticon: "👍".into() }));
        let some = types::ChatReactionsSome { reactions: vec![like, base::Reaction::ReactionPaid] };
        let some = ChatReactions::parse(&base::ChatReactions::ChatReactionsSome(some.into()));
        assert_eq!(some.reactions, [ReactionType::Emoji("👍".into()), ReactionType::Paid]);
        assert!(!some.all_are_enabled);

        let none = ChatReactions::parse(&base::ChatReactions::ChatReactionsNone);
        assert_eq!(none, ChatReactions::default());
    }
}
