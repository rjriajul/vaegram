use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::UserStatus;
use crate::raw::base;
use crate::types::{EmojiStatus, RestrictionReason};

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
    /// This is your own account.
    pub is_self: bool,
    /// In your contacts.
    pub is_contact: bool,
    /// You are in each other's contacts.
    pub is_mutual_contact: bool,
    /// Hidden on some platforms; see `restrictions`.
    pub is_restricted: bool,
    /// An official Telegram support account.
    pub is_support: bool,
    /// Only Premium users can message them.
    pub is_contact_require_premium: bool,
    /// In your close friends list.
    pub is_close_friend: bool,
    /// You hid their stories.
    pub is_stories_hidden: bool,
    pub is_stories_unavailable: bool,
    /// The custom emoji shown next to a verified bot's name.
    pub bot_verification_icon_custom_emoji_id: Option<i64>,
    /// Visible only when they share it with you.
    pub phone_number: Option<String>,
    /// Bots only: you added it to your attachment menu.
    pub added_to_attachment_menu: bool,
    /// Bots only: monthly active users.
    pub active_users_count: Option<i32>,
    /// Bots only: inline queries ask for the user's location.
    pub inline_need_location: bool,
    /// Bots only: the hint shown in the inline query field.
    pub inline_query_placeholder: Option<String>,
    /// Bots only: you own this bot.
    pub can_be_edited: bool,
    /// Bots only: the bot offers an attachment menu entry.
    pub can_be_added_to_attachment_menu: bool,
    /// Stars charged per message sent to them.
    pub paid_message_star_count: Option<i64>,
    pub emoji_status: Option<EmojiStatus>,
    pub restrictions: Vec<RestrictionReason>,
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
                is_self: u.is_self,
                is_contact: u.contact,
                is_mutual_contact: u.mutual_contact,
                is_restricted: u.restricted,
                is_support: u.support,
                is_contact_require_premium: u.contact_require_premium,
                is_close_friend: u.close_friend,
                is_stories_hidden: u.stories_hidden,
                is_stories_unavailable: u.stories_unavailable,
                bot_verification_icon_custom_emoji_id: u.bot_verification_icon,
                phone_number: u.phone.clone(),
                added_to_attachment_menu: u.attach_menu_enabled,
                active_users_count: u.bot_active_users,
                inline_need_location: u.bot_inline_geo,
                inline_query_placeholder: u.bot_inline_placeholder.clone(),
                can_be_edited: u.bot_can_edit,
                can_be_added_to_attachment_menu: u.bot_attach_menu,
                paid_message_star_count: u.send_paid_messages_stars,
                emoji_status: u.emoji_status.as_ref().and_then(EmojiStatus::parse),
                restrictions: u.restriction_reason.iter().flatten().map(RestrictionReason::parse).collect(),
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

    fn raw(id: i64) -> types::User {
        types::User {
            is_self: false,
            contact: false,
            mutual_contact: false,
            deleted: false,
            bot: false,
            bot_chat_history: false,
            bot_nochats: false,
            verified: false,
            restricted: false,
            min: false,
            bot_inline_geo: false,
            support: false,
            scam: false,
            apply_min_photo: false,
            fake: false,
            bot_attach_menu: false,
            premium: false,
            attach_menu_enabled: false,
            bot_can_edit: false,
            close_friend: false,
            stories_hidden: false,
            stories_unavailable: false,
            contact_require_premium: false,
            bot_business: false,
            bot_has_main_app: false,
            bot_forum_view: false,
            bot_forum_can_manage_topics: false,
            bot_can_manage_bots: false,
            bot_guestchat: false,
            bot_guard: false,
            id,
            access_hash: None,
            first_name: Some("Neko".into()),
            last_name: None,
            username: None,
            phone: None,
            photo: None,
            status: None,
            bot_info_version: None,
            restriction_reason: None,
            bot_inline_placeholder: None,
            lang_code: None,
            emoji_status: None,
            usernames: None,
            stories_max_id: None,
            color: None,
            profile_color: None,
            bot_active_users: None,
            bot_verification_icon: None,
            send_paid_messages_stars: None,
            linked_community_id: None,
        }
    }

    fn parse(raw: types::User) -> User {
        let id = raw.id;
        let mut entities = Entities::default();
        entities.users.insert(id, base::User::User(Box::new(raw)));
        User::parse(id, &entities)
    }

    #[test]
    fn a_user_with_no_optional_fields_equals_the_default() {
        assert_eq!(parse(raw(7)), User { id: 7, first_name: "Neko".into(), ..User::default() });
    }

    #[test]
    fn reads_contact_and_bot_flags() {
        type Set = fn(&mut types::User);
        type Get = fn(&User) -> bool;
        let cases: [(Set, Get); 12] = [
            (|u| u.is_self = true, |u| u.is_self),
            (|u| u.contact = true, |u| u.is_contact),
            (|u| u.mutual_contact = true, |u| u.is_mutual_contact),
            (|u| u.restricted = true, |u| u.is_restricted),
            (|u| u.support = true, |u| u.is_support),
            (|u| u.contact_require_premium = true, |u| u.is_contact_require_premium),
            (|u| u.close_friend = true, |u| u.is_close_friend),
            (|u| u.stories_hidden = true, |u| u.is_stories_hidden),
            (|u| u.stories_unavailable = true, |u| u.is_stories_unavailable),
            (|u| u.attach_menu_enabled = true, |u| u.added_to_attachment_menu),
            (|u| u.bot_inline_geo = true, |u| u.inline_need_location),
            (|u| u.bot_can_edit = true, |u| u.can_be_edited),
        ];
        for (i, (set, get)) in cases.iter().enumerate() {
            let mut user = raw(7);
            set(&mut user);
            let user = parse(user);
            assert!(get(&user), "case {i}");
            for (j, (_, other)) in cases.iter().enumerate() {
                assert_eq!(other(&user), i == j, "case {i} sets {j}");
            }
        }
        let mut bot = raw(7);
        bot.bot_attach_menu = true;
        assert!(parse(bot).can_be_added_to_attachment_menu);
    }

    #[test]
    fn reads_phone_restrictions_paid_messages_and_emoji_status() {
        let mut user = raw(7);
        user.phone = Some("15550001".into());
        user.send_paid_messages_stars = Some(250);
        user.bot_active_users = Some(42);
        user.bot_verification_icon = Some(11);
        user.bot_inline_placeholder = Some("Search".into());
        let reason = types::RestrictionReason { platform: "ios".into(), reason: "porn".into(), text: "hidden".into() };
        user.restriction_reason = Some(vec![base::RestrictionReason::RestrictionReason(reason.into())]);
        let status = types::EmojiStatus { document_id: 5, until: None };
        user.emoji_status = Some(base::EmojiStatus::EmojiStatus(status.into()));
        let user = parse(user);
        assert_eq!(user.phone_number.as_deref(), Some("15550001"));
        assert_eq!(user.paid_message_star_count, Some(250));
        assert_eq!(user.active_users_count, Some(42));
        assert_eq!(user.bot_verification_icon_custom_emoji_id, Some(11));
        assert_eq!(user.inline_query_placeholder.as_deref(), Some("Search"));
        assert!(user.supports_inline_queries);
        assert_eq!(user.restrictions.len(), 1);
        assert_eq!((user.restrictions[0].platform.as_str(), user.restrictions[0].reason.as_str()), ("ios", "porn"));
        assert_eq!(user.emoji_status.map(|s| s.custom_emoji_id), Some(5));
        let mut empty_status = raw(7);
        empty_status.emoji_status = Some(base::EmojiStatus::EmojiStatusEmpty);
        assert_eq!(parse(empty_status).emoji_status, None);
    }
}
