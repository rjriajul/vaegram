use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::User;

#[derive(Debug, Clone)]
pub struct UserStatusUpdated {
    pub user: User,
    client: Client,
}

impl UserStatusUpdated {
    pub(crate) fn parse(client: &Client, raw: &types::UpdateUserStatus, entities: &Entities) -> UserStatusUpdated {
        let user = User::parse(raw.user_id, entities).parse_status(Some(&raw.status));
        UserStatusUpdated { user, client: client.clone() }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }
}

#[cfg(test)]
mod tests {
    use super::UserStatusUpdated;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::enums::UserStatus;
    use crate::raw::{base, types};

    #[test]
    fn parses_offline_status() {
        let status = base::UserStatus::UserStatusOffline(Box::new(types::UserStatusOffline { was_online: 100 }));
        let raw = types::UpdateUserStatus { user_id: 7, status };
        let user = UserStatusUpdated::parse(&Client::offline(), &raw, &Entities::default()).user;
        assert_eq!((user.id, user.status), (7, Some(UserStatus::Offline)));
        assert_eq!((user.last_online_date, user.next_offline_date), (Some(100), None));
    }
}
