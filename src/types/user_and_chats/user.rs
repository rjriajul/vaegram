use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::UserStatus;
use crate::raw::base;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct User {
    pub id: i64,
    pub is_bot: bool,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub language_code: Option<String>,
    pub is_premium: bool,
    pub is_deleted: bool,
    pub is_verified: bool,
    pub is_scam: bool,
    pub is_fake: bool,
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
    pub status: Option<UserStatus>,
    pub last_online_date: Option<i32>,
    pub next_offline_date: Option<i32>,
}

impl User {
    pub(crate) fn from_raw(client: &Client, user: base::User) -> User {
        let id = match &user {
            base::User::User(u) => u.id,
            base::User::UserEmpty(u) => u.id,
        };
        User::parse(id, &Entities::new(client, vec![user], Vec::new()))
    }

    pub(crate) fn parse(id: i64, entities: &Entities) -> User {
        match entities.users.get(&id) {
            Some(base::User::User(u)) => User {
                id,
                is_bot: u.bot,
                first_name: u.first_name.clone().unwrap_or_default(),
                last_name: u.last_name.clone(),
                username: u.username.clone(),
                language_code: u.lang_code.clone(),
                is_premium: u.premium,
                is_deleted: u.deleted,
                is_verified: u.verified,
                is_scam: u.scam,
                is_fake: u.fake,
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
                ..User::default().parse_status(u.status.as_ref())
            },
            _ => User { id, ..User::default() },
        }
    }

    pub(crate) fn parse_status(mut self, status: Option<&base::UserStatus>) -> User {
        use base::UserStatus as S;
        (self.status, self.last_online_date, self.next_offline_date) = match status {
            None => (None, None, None),
            Some(S::UserStatusOnline(s)) => (Some(UserStatus::Online), None, Some(s.expires)),
            Some(S::UserStatusOffline(s)) => (Some(UserStatus::Offline), Some(s.was_online), None),
            Some(S::UserStatusRecently(_)) => (Some(UserStatus::Recently), None, None),
            Some(S::UserStatusLastWeek(_)) => (Some(UserStatus::LastWeek), None, None),
            Some(S::UserStatusLastMonth(_)) => (Some(UserStatus::LastMonth), None, None),
            Some(S::UserStatusEmpty) => (Some(UserStatus::LongAgo), None, None),
        };
        self
    }

    pub(crate) fn parse_all(client: &Client, users: Vec<base::User>) -> Vec<User> {
        let ids: Vec<i64> = users
            .iter()
            .filter_map(|u| match u {
                base::User::User(u) => Some(u.id),
                base::User::UserEmpty(_) => None,
            })
            .collect();
        let entities = Entities::new(client, users, Vec::new());
        ids.into_iter().map(|id| User::parse(id, &entities)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::User;
    use crate::dispatcher::Entities;
    use crate::raw::{Serializable, base, types};

    #[test]
    fn reads_account_flags_and_language() {
        // deleted 13, verified 17, lang_code 22, scam 24, fake 26, premium 28; no flags2.
        let flags: u32 = 1 << 13 | 1 << 17 | 1 << 22 | 1 << 24 | 1 << 26 | 1 << 28;
        let mut buf = Vec::new();
        for word in [flags, 0] {
            word.serialize(&mut buf);
        }
        7i64.serialize(&mut buf);
        String::from("pt-br").serialize(&mut buf);
        let raw = types::User::deserialize_bare(&mut &buf[..]).unwrap();
        let mut entities = Entities::default();
        entities.users.insert(7, base::User::User(Box::new(raw)));
        let user = User::parse(7, &entities);
        assert_eq!(user.language_code.as_deref(), Some("pt-br"));
        assert!(user.is_premium && user.is_deleted && user.is_verified && user.is_scam && user.is_fake);
        assert!(!User::parse(8, &entities).is_premium);
    }
}
