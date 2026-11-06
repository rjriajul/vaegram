use crate::raw::base;

/// An emoji reaction that Telegram offers for messages.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct AvailableReaction {
    /// The reaction's emoji.
    pub emoji: String,
    /// Name of the reaction.
    pub title: String,
    /// `true` if only Premium users can use the reaction.
    pub is_premium: bool,
    /// `true` if the reaction can no longer be added to messages.
    pub is_inactive: bool,
}

impl AvailableReaction {
    pub(crate) fn parse(raw: base::AvailableReaction) -> AvailableReaction {
        let base::AvailableReaction::AvailableReaction(r) = raw;
        AvailableReaction { emoji: r.reaction, title: r.title, is_premium: r.premium, is_inactive: r.inactive }
    }
}
