use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::User;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatSettings {
    pub can_report_spam: bool,
    pub can_add_contact: bool,
    pub can_block_contact: bool,
    pub can_share_contact: bool,
    pub need_contacts_exception: bool,
    pub is_autoarchived: bool,
    pub can_invite_members: bool,
    pub is_business_bot_paused: bool,
    pub can_business_bot_reply: bool,
    pub request_chat_is_channel: bool,
    pub request_chat_title: Option<String>,
    pub request_chat_date: Option<i32>,
    pub business_bot: Option<User>,
    pub business_bot_manage_url: Option<String>,
    pub paid_message_star_count: Option<i64>,
    pub registration_month: Option<String>,
    pub phone_country: Option<String>,
    pub name_change_date: Option<i32>,
    pub photo_change_date: Option<i32>,
}

impl ChatSettings {
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
        assert_eq!(ChatSettings::parse(empty(), &entities), ChatSettings::default());
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
        assert_eq!(settings.business_bot, Some(User { id: 42, ..User::default() }));
    }
}
