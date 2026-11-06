use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::User;

/// What Telegram suggests doing with a chat, and what it knows about the other side.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ChatSettings {
    /// Whether the chat can be reported as spam.
    pub can_report_spam: bool,
    /// Whether the user can be added to contacts.
    pub can_add_contact: bool,
    /// Whether the user can be blocked.
    pub can_block_contact: bool,
    /// Whether the account's phone number can be shared with the user.
    pub can_share_contact: bool,
    /// Whether adding the user to contacts needs an exception to their privacy settings.
    pub need_contacts_exception: bool,
    /// Whether Telegram moved the chat to the archive on its own.
    pub is_autoarchived: bool,
    /// Whether the account can invite members to the group.
    pub can_invite_members: bool,
    /// Whether the connected business bot is paused in this chat.
    pub is_business_bot_paused: bool,
    /// Whether the connected business bot can reply in this chat.
    pub can_business_bot_reply: bool,
    /// Whether the chat in `request_chat_title` is a channel rather than a group.
    pub request_chat_is_channel: bool,
    /// Title of the chat the account asked to join, in a private chat with one of its admins.
    pub request_chat_title: Option<String>,
    /// Unix time the account sent the join request named in `request_chat_title`.
    pub request_chat_date: Option<i32>,
    /// The business bot connected to this chat. `None` when there is none.
    pub business_bot: Option<User>,
    /// Link to manage the connected business bot.
    pub business_bot_manage_url: Option<String>,
    /// Stars the account pays for each message it sends here.
    pub paid_message_star_count: Option<i64>,
    /// Month the other user registered, as Telegram sends it.
    pub registration_month: Option<String>,
    /// Country of the other user's phone number.
    pub phone_country: Option<String>,
    /// Unix time the other user last changed their name.
    pub name_change_date: Option<i32>,
    /// Unix time the other user last changed their photo.
    pub photo_change_date: Option<i32>,
}

impl ChatSettings {
    /// Every field zero, empty or `None`: the base the parsers and tests fill in.
    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        ChatSettings {
            can_report_spam: false,
            can_add_contact: false,
            can_block_contact: false,
            can_share_contact: false,
            need_contacts_exception: false,
            is_autoarchived: false,
            can_invite_members: false,
            is_business_bot_paused: false,
            can_business_bot_reply: false,
            request_chat_is_channel: false,
            request_chat_title: None,
            request_chat_date: None,
            business_bot: None,
            business_bot_manage_url: None,
            paid_message_star_count: None,
            registration_month: None,
            phone_country: None,
            name_change_date: None,
            photo_change_date: None,
        }
    }

    pub(crate) fn parse(raw: types::PeerSettings, entities: &Entities) -> ChatSettings {
        ChatSettings {
            can_report_spam: raw.report_spam,
            can_add_contact: raw.add_contact,
            can_block_contact: raw.block_contact,
            can_share_contact: raw.share_contact,
            need_contacts_exception: raw.need_contacts_exception,
            is_autoarchived: raw.autoarchived,
            can_invite_members: raw.invite_members,
            is_business_bot_paused: raw.business_bot_paused,
            can_business_bot_reply: raw.business_bot_can_reply,
            request_chat_is_channel: raw.request_chat_broadcast,
            request_chat_title: raw.request_chat_title,
            request_chat_date: raw.request_chat_date,
            business_bot: raw.business_bot_id.map(|id| User::parse(id, entities)),
            business_bot_manage_url: raw.business_bot_manage_url,
            paid_message_star_count: raw.charge_paid_message_stars,
            registration_month: raw.registration_month,
            phone_country: raw.phone_country,
            name_change_date: raw.name_change_date,
            photo_change_date: raw.photo_change_date,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ChatSettings;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::raw::{Serializable, base, types};
    use crate::types::User;

    fn empty() -> types::PeerSettings {
        // A flags word of zero: no setting is present.
        types::PeerSettings::deserialize_bare(&mut &[0u8; 4][..]).unwrap()
    }

    fn bot(id: i64) -> base::User {
        let mut buf = Vec::new();
        // Only access_hash and first_name are set.
        for word in [0b11u32, 0] {
            word.serialize(&mut buf);
        }
        id.serialize(&mut buf);
        (id * 10).serialize(&mut buf);
        String::from("Helper").serialize(&mut buf);
        base::User::User(Box::new(types::User::deserialize_bare(&mut &buf[..]).unwrap()))
    }

    #[test]
    fn no_action_bar_is_the_default() {
        let entities = Entities::default();
        assert_eq!(ChatSettings::parse(empty(), &entities), ChatSettings::empty());
    }

    #[test]
    fn every_setting_is_mapped() {
        let raw = types::PeerSettings {
            report_spam: true,
            add_contact: true,
            block_contact: true,
            share_contact: true,
            need_contacts_exception: true,
            report_geo: true,
            autoarchived: true,
            invite_members: true,
            request_chat_broadcast: true,
            business_bot_paused: true,
            business_bot_can_reply: true,
            geo_distance: Some(500),
            request_chat_title: Some("News".into()),
            request_chat_date: Some(100),
            business_bot_id: Some(42),
            business_bot_manage_url: Some("https://t.me/x".into()),
            charge_paid_message_stars: Some(25),
            registration_month: Some("01.2020".into()),
            phone_country: Some("DE".into()),
            name_change_date: Some(200),
            photo_change_date: Some(300),
        };
        let entities = Entities::new(&Client::offline(), vec![bot(42)], Vec::new());
        let settings = ChatSettings::parse(raw, &entities);
        let expected = ChatSettings {
            can_report_spam: true,
            can_add_contact: true,
            can_block_contact: true,
            can_share_contact: true,
            need_contacts_exception: true,
            is_autoarchived: true,
            can_invite_members: true,
            is_business_bot_paused: true,
            can_business_bot_reply: true,
            request_chat_is_channel: true,
            request_chat_title: Some("News".into()),
            request_chat_date: Some(100),
            business_bot: Some(User::parse(42, &entities)),
            business_bot_manage_url: Some("https://t.me/x".into()),
            paid_message_star_count: Some(25),
            registration_month: Some("01.2020".into()),
            phone_country: Some("DE".into()),
            name_change_date: Some(200),
            photo_change_date: Some(300),
        };
        assert_eq!(settings, expected);
        assert_eq!(settings.business_bot.map(|b| b.first_name), Some("Helper".into()));
    }

    #[test]
    fn a_business_bot_missing_from_users_keeps_its_id() {
        let raw = types::PeerSettings { business_bot_id: Some(42), ..empty() };
        let settings = ChatSettings::parse(raw, &Entities::default());
        assert_eq!(settings.business_bot, Some(User { id: 42, ..User::empty() }));
    }
}
