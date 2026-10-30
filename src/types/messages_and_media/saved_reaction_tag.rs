use crate::raw::base;
use crate::types::ReactionType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedReactionTag {
    pub reaction: ReactionType,
    pub title: Option<String>,
    pub count: i32,
}

impl SavedReactionTag {
    pub(crate) fn parse(raw: base::SavedReactionTag) -> Option<SavedReactionTag> {
        let base::SavedReactionTag::SavedReactionTag(t) = raw;
        Some(SavedReactionTag { reaction: ReactionType::parse(&t.reaction)?, title: t.title, count: t.count })
    }
}
