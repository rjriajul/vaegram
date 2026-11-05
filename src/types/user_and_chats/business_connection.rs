use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::{BusinessBotRights, User};

/// A bot's connection to a business account.
#[derive(Debug, Clone)]
pub struct BusinessConnection {
    /// Id of the connection.
    pub id: String,
    /// The business account.
    pub user: User,
    /// Unix time the connection was made.
    pub date: i32,
    /// What the bot may do in the account. All `false` if Telegram sent no rights.
    pub rights: BusinessBotRights,
    /// `false` once the account has disabled the connection.
    pub is_enabled: bool,
    /// The data center of the connected account.
    pub dc_id: i32,
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
            dc_id: raw.dc_id,
            id: raw.connection_id,
            client: client.clone(),
        }
    }

    /// Returns the client that received this connection.
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

    #[test]
    fn keeps_the_dc() {
        let raw = base::BotBusinessConnection::BotBusinessConnection(Box::new(types::BotBusinessConnection {
            disabled: false,
            connection_id: "c".into(),
            user_id: 7,
            dc_id: 4,
            date: 1,
            rights: None,
        }));
        let c = BusinessConnection::parse(&Client::offline(), raw, &Entities::default());
        assert_eq!((c.dc_id, c.is_enabled), (4, true));
    }
}
