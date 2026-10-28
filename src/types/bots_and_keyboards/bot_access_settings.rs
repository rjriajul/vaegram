use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::User;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotAccessSettings {
    pub is_access_restricted: bool,
    pub added_users: Vec<User>,
}

impl BotAccessSettings {
    pub(crate) fn parse(client: &Client, raw: base::bots::AccessSettings) -> BotAccessSettings {
        let base::bots::AccessSettings::AccessSettings(raw) = raw;
        let users = raw.add_users.unwrap_or_default();
        let ids: Vec<i64> = users
            .iter()
            .filter_map(|u| match u {
                base::User::User(u) => Some(u.id),
                _ => None,
            })
            .collect();
        let entities = Entities::new(client, users, Vec::new());
        BotAccessSettings {
            is_access_restricted: raw.restricted,
            added_users: ids.into_iter().map(|id| User::parse(id, &entities)).collect(),
        }
    }
}
