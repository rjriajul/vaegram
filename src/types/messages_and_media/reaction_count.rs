use crate::raw::base;
use crate::types::ReactionType;

/// How many times one reaction was used on a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReactionCount {
    /// The reaction being counted.
    pub kind: ReactionType,
    /// Number of times the reaction was put on the message.
    pub total_count: i32,
    /// Position of this reaction among the ones the current account picked. Lower values were picked
    /// earlier. `None` if the account didn't pick it.
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
