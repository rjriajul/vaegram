use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::User;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatJoiner {
    pub user: User,
    pub date: i32,
    pub bio: Option<String>,
}

impl ChatJoiner {
    pub(crate) fn parse(raw: types::ChatInviteImporter, entities: &Entities) -> ChatJoiner {
        ChatJoiner { user: User::parse(raw.user_id, entities), date: raw.date, bio: raw.about }
    }
}
