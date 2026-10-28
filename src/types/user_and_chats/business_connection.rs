use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::{BusinessBotRights, User};

#[derive(Debug, Clone)]
pub struct BusinessConnection {
    pub id: String,
    pub user: User,
    pub date: i32,
    pub rights: BusinessBotRights,
    pub is_enabled: bool,
    client: Client,
}

impl BusinessConnection {
    pub(crate) fn parse(client: &Client, raw: base::BotBusinessConnection, entities: &Entities) -> BusinessConnection {
        let base::BotBusinessConnection::BotBusinessConnection(raw) = raw;
        BusinessConnection {
            user: User::parse(raw.user_id, entities),
            date: raw.date,
            rights: raw.rights.as_ref().map(BusinessBotRights::parse).unwrap_or_default(),
            is_enabled: !raw.disabled,
            id: raw.connection_id,
            client: client.clone(),
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }
}

#[cfg(test)]
mod tests {
    use super::BusinessConnection;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};

    #[test]
    fn maps_renamed_rights() {
        let rights = types::BusinessBotRights {
            reply: true,
            read_messages: false,
            delete_sent_messages: false,
            delete_received_messages: true,
            edit_name: false,
            edit_bio: false,
            edit_profile_photo: false,
            edit_username: false,
            view_gifts: true,
            sell_gifts: true,
            change_gift_settings: false,
            transfer_and_upgrade_gifts: false,
            transfer_stars: false,
            manage_stories: false,
        };
        let raw = base::BotBusinessConnection::BotBusinessConnection(Box::new(types::BotBusinessConnection {
            disabled: true,
            connection_id: "c".into(),
            user_id: 7,
            dc_id: 2,
            date: 1,
            rights: Some(base::BusinessBotRights::BusinessBotRights(Box::new(rights))),
        }));
        let c = BusinessConnection::parse(&Client::offline(), raw, &Entities::default());
        assert_eq!((c.id.as_str(), c.is_enabled, c.user.id), ("c", false, 7));
        let r = c.rights;
        assert_eq!(
            (
                r.can_reply,
                r.can_read_messages,
                r.can_delete_sent_messages,
                r.can_delete_all_messages,
                r.can_view_gifts_and_stars,
                r.can_convert_gifts_to_stars
            ),
            (true, false, false, true, true, true)
        );
    }
}
