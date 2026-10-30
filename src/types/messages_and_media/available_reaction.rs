use crate::raw::base;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableReaction {
    pub emoji: String,
    pub title: String,
    pub premium: bool,
    pub inactive: bool,
}

impl AvailableReaction {
    pub(crate) fn parse(raw: base::AvailableReaction) -> AvailableReaction {
        let base::AvailableReaction::AvailableReaction(r) = raw;
        AvailableReaction { emoji: r.reaction, title: r.title, premium: r.premium, inactive: r.inactive }
    }
}
