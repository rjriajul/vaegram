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
    pub supports_guest_queries: bool,
    pub can_join_groups: bool,
    pub can_read_all_group_messages: bool,
    pub supports_inline_queries: bool,
    pub can_connect_to_business: bool,
    pub has_main_web_app: bool,
    pub can_manage_bots: bool,
    pub has_topics_enabled: bool,
    pub allows_users_to_create_topics: bool,
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
                supports_guest_queries: u.bot_guestchat,
                can_join_groups: u.bot && !u.bot_nochats,
                can_read_all_group_messages: u.bot_chat_history,
                supports_inline_queries: u.bot_inline_placeholder.is_some(),
                can_connect_to_business: u.bot_business,
                has_main_web_app: u.bot_has_main_app,
                can_manage_bots: u.bot_can_manage_bots,
                has_topics_enabled: u.bot_forum_view,
                allows_users_to_create_topics: u.bot_forum_can_manage_topics,
            },
            _ => User { id, ..User::default() },
        }
    }
}
