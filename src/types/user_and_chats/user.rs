use crate::dispatcher::Entities;
use crate::raw::base;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct User {
    pub id: i64,
    pub is_bot: bool,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub supports_join_request_queries: bool,
}

impl User {
    pub(crate) fn parse(id: i64, entities: &Entities) -> User {
        match entities.users.get(&id) {
            Some(base::User::User(u)) => User {
                id,
                is_bot: u.bot,
                first_name: u.first_name.clone().unwrap_or_default(),
                last_name: u.last_name.clone(),
                username: u.username.clone(),
                // Inferred from the name: api.tl does not describe bot_guard.
                supports_join_request_queries: u.bot_guard,
            },
            _ => User { id, ..User::default() },
        }
    }
}
