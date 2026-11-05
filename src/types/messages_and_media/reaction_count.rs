use crate::raw::base;
use crate::types::ReactionType;

/// How many times one reaction was used on a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReactionCount {
    /// The reaction.
    pub kind: ReactionType,
    /// Number of times it was used.
    pub total_count: i32,
    /// Set when the current account picked this reaction; lower values were picked earlier.
    pub chosen_order: Option<i32>,
}

impl ReactionCount {
    pub(crate) fn parse(raw: &base::ReactionCount) -> Option<ReactionCount> {
        let base::ReactionCount::ReactionCount(r) = raw;
        Some(ReactionCount {
            kind: ReactionType::parse(&r.reaction)?,
            total_count: r.count,
            chosen_order: r.chosen_order,
        })
    }
}
