use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::User;

/// A user who joined a chat through an invite link.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ChatJoiner {
    /// The user who joined.
    pub user: User,
    /// Unix time the user joined.
    pub date: i32,
    /// The user's bio. `None` if Telegram didn't send one.
    pub bio: Option<String>,
}

impl ChatJoiner {
    pub(crate) fn parse(raw: types::ChatInviteImporter, entities: &Entities) -> ChatJoiner {
        ChatJoiner { user: User::parse(raw.user_id, entities), date: raw.date, bio: raw.about }
    }
}
