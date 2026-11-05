use crate::raw::base;
use crate::types::ReactionType;

/// A reaction the account saved as a tag for its messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedReactionTag {
    /// The reaction used as the tag.
    pub reaction: ReactionType,
    /// The name given to the tag, if any.
    pub title: Option<String>,
    /// Number of messages with the tag.
    pub count: i32,
}

impl SavedReactionTag {
    pub(crate) fn parse(raw: base::SavedReactionTag) -> Option<SavedReactionTag> {
        let base::SavedReactionTag::SavedReactionTag(t) = raw;
        Some(SavedReactionTag { reaction: ReactionType::parse(&t.reaction)?, title: t.title, count: t.count })
    }
}
