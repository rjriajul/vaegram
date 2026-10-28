use crate::raw::{base, types};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ReactionType {
    Emoji(String),
    CustomEmoji(i64),
    Paid,
}

impl ReactionType {
    pub(crate) fn parse(raw: &base::Reaction) -> Option<ReactionType> {
        match raw {
            base::Reaction::ReactionEmoji(r) => Some(ReactionType::Emoji(r.emoticon.clone())),
            base::Reaction::ReactionCustomEmoji(r) => Some(ReactionType::CustomEmoji(r.document_id)),
            base::Reaction::ReactionPaid => Some(ReactionType::Paid),
            base::Reaction::ReactionEmpty => None,
        }
    }

    pub(crate) fn write(&self) -> base::Reaction {
        match self {
            ReactionType::Emoji(emoticon) => {
                base::Reaction::ReactionEmoji(Box::new(types::ReactionEmoji { emoticon: emoticon.clone() }))
            }
            ReactionType::CustomEmoji(document_id) => {
                base::Reaction::ReactionCustomEmoji(Box::new(types::ReactionCustomEmoji { document_id: *document_id }))
            }
            ReactionType::Paid => base::Reaction::ReactionPaid,
        }
    }
}

impl From<&str> for ReactionType {
    fn from(emoji: &str) -> Self {
        ReactionType::Emoji(emoji.into())
    }
}
