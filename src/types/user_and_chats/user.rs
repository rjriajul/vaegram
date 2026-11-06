use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::UserStatus;
use crate::raw::base;
use crate::types::{ChatColor, ChatPhoto, EmojiStatus, RestrictionReason, Username};

/// A Telegram user or bot.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct User {
    /// Unique id of the user. Always positive.
    pub id: i64,
    /// The user is a bot.
    pub is_bot: bool,
    /// First name. Empty if Telegram sent no name or no user object.
    pub first_name: String,
    /// Last name. `None` if not set.
    pub last_name: Option<String>,
    /// Main username, or the first active entry of `usernames` when Telegram sends no main one.
    pub username: Option<String>,
    /// Language code of the user's app, when Telegram sends it.
    pub language_code: Option<String>,
    /// The user has Telegram Premium.
    pub is_premium: bool,
    /// The account is deleted.
    pub is_deleted: bool,
    /// Verified by Telegram.
    pub is_verified: bool,
    /// Telegram flagged the account as a scam.
    pub is_scam: bool,
    /// Telegram flagged the account as impersonating someone.
    pub is_fake: bool,
    /// Bots only: the bot handles join request queries, as read from Telegram's `bot_guard` flag.
    pub supports_join_request_queries: bool,
    /// Bots only: the bot handles guest chat queries.
    pub supports_guest_queries: bool,
    /// Bots only: the bot can be added to groups.
    pub can_join_groups: bool,
    /// Bots only: the bot sees all messages in groups, not only commands and replies.
    pub can_read_all_group_messages: bool,
    /// Bots only: the bot handles inline queries.
    pub supports_inline_queries: bool,
    /// Bots only: the bot can be connected to a business account.
    pub can_connect_to_business: bool,
    /// Bots only: the bot has a main Mini App.
    pub has_main_web_app: bool,
    /// Bots only: the bot can manage other bots.
    pub can_manage_bots: bool,
    /// Bots only: the bot chat shows topics.
    pub has_topics_enabled: bool,
    /// Bots only: users can create topics in the bot chat.
    pub allows_users_to_create_topics: bool,
    /// Bots only: the bot chat shows its topics in a sidebar.
    pub has_topics_sidebar: bool,
    /// Online status. `None` when Telegram did not send it.
    pub status: Option<UserStatus>,
    /// Unix time the user was last online. Set only when `status` is offline.
    pub last_online_date: Option<i32>,
    /// Unix time the user's online status expires. Set only when `status` is online.
    pub next_offline_date: Option<i32>,
    /// This is your own account.
    pub is_self: bool,
    /// In your contacts.
    pub is_contact: bool,
    /// You are in each other's contacts.
    pub is_mutual_contact: bool,
    /// Hidden on some platforms. `restrictions` says where and why.
    pub is_restricted: bool,
    /// An official Telegram support account.
    pub is_support: bool,
    /// Only Premium users can message them.
    pub is_contact_require_premium: bool,
    /// In your close friends list.
    pub is_close_friend: bool,
    /// You hid their stories.
    pub is_stories_hidden: bool,
    /// Their stories are unavailable to you.
    pub is_stories_unavailable: bool,
    /// Custom emoji id of the bot verification icon shown next to the name.
    pub bot_verification_icon_custom_emoji_id: Option<i64>,
    /// Phone number. `None` unless they share it with you.
    pub phone_number: Option<String>,
    /// Bots only: you added it to your attachment menu.
    pub added_to_attachment_menu: bool,
    /// Bots only: monthly active users.
    pub active_users_count: Option<i32>,
    /// Bots only: inline queries ask for the user's location.
    pub inline_needs_location: bool,
    /// Bots only: the hint shown in the inline query field.
    pub inline_query_placeholder: Option<String>,
    /// Bots only: you own this bot.
    pub can_be_edited: bool,
    /// Bots only: the bot offers an attachment menu entry.
    pub can_be_added_to_attachment_menu: bool,
    /// Stars charged per message sent to them. `None` when messages are free.
    pub paid_message_star_count: Option<i64>,
    /// The emoji status. `None` if not set.
    pub emoji_status: Option<EmojiStatus>,
    /// Why the account is restricted on some platforms.
    pub restrictions: Vec<RestrictionReason>,
    /// Every username, including collectible ones.
    pub usernames: Vec<Username>,
    /// The data center that stores the profile photo. `None` without a photo.
    pub dc_id: Option<i32>,
    /// The profile photo. `None` if there is none or Telegram sent no access hash for the user.
    pub photo: Option<ChatPhoto>,
    /// Color used for replies to their messages. `None` if not set.
    pub reply_color: Option<ChatColor>,
    /// Color of the profile page. `None` if not set.
    pub profile_color: Option<ChatColor>,
}

impl User {
    /// Every field zero, empty or `None`: the base the parsers and tests fill in.
    pub(crate) fn empty() -> Self {
        User {
            id: 0,
            is_bot: false,
            first_name: String::new(),
            last_name: None,
            username: None,
            language_code: None,
            is_premium: false,
            is_deleted: false,
            is_verified: false,
            is_scam: false,
            is_fake: false,
            supports_join_request_queries: false,
            supports_guest_queries: false,
            can_join_groups: false,
            can_read_all_group_messages: false,
            supports_inline_queries: false,
            can_connect_to_business: false,
            has_main_web_app: false,
            can_manage_bots: false,
            has_topics_enabled: false,
            allows_users_to_create_topics: false,
            has_topics_sidebar: false,
            status: None,
            last_online_date: None,
            next_offline_date: None,
            is_self: false,
            is_contact: false,
            is_mutual_contact: false,
            is_restricted: false,
            is_support: false,
            is_contact_require_premium: false,
            is_close_friend: false,
            is_stories_hidden: false,
            is_stories_unavailable: false,
            bot_verification_icon_custom_emoji_id: None,
            phone_number: None,
            added_to_attachment_menu: false,
            active_users_count: None,
            inline_needs_location: false,
            inline_query_placeholder: None,
            can_be_edited: false,
            can_be_added_to_attachment_menu: false,
            paid_message_star_count: None,
            emoji_status: None,
            restrictions: Vec::new(),
            usernames: Vec::new(),
            dc_id: None,
            photo: None,
            reply_color: None,
            profile_color: None,
        }
    }

    pub(crate) fn from_raw(client: &Client, user: base::User) -> User {
        let id = match &user {
            base::User::User(u) => u.id,
            base::User::UserEmpty(u) => u.id,
        };
        User::parse(id, &Entities::new(client, vec![user], Vec::new()))
    }

    pub(crate) fn parse(id: i64, entities: &Entities) -> User {
        match entities.users.get(&id) {
            Some(base::User::User(u)) => {
                let usernames: Vec<Username> = u.usernames.iter().flatten().map(Username::parse).collect();
                let profile_photo = match &u.photo {
                    Some(base::UserProfilePhoto::UserProfilePhoto(p)) => Some(p),
                    _ => None,
                };
                User {
                    id,
                    is_bot: u.bot,
                    first_name: u.first_name.clone().unwrap_or_default(),
                    last_name: u.last_name.clone(),
                    username: u
                        .username
                        .clone()
                        .or_else(|| usernames.iter().find(|n| n.is_active).map(|n| n.username.clone())),
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
                    has_topics_sidebar: u.bot_forum_sidebar,
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
                    inline_needs_location: u.bot_inline_geo,
                    inline_query_placeholder: u.bot_inline_placeholder.clone(),
                    can_be_edited: u.bot_can_edit,
                    can_be_added_to_attachment_menu: u.bot_attach_menu,
                    paid_message_star_count: u.send_paid_messages_stars,
                    emoji_status: u.emoji_status.as_ref().and_then(EmojiStatus::parse),
                    restrictions: u.restriction_reason.iter().flatten().map(RestrictionReason::parse).collect(),
                    usernames,
                    dc_id: profile_photo.map(|p| p.dc_id),
                    photo: profile_photo
                        .zip(u.access_hash)
                        .map(|(p, hash)| ChatPhoto::new(id, hash, p.photo_id, p.dc_id, p.has_video, p.personal)),
                    reply_color: u.color.as_ref().and_then(ChatColor::parse),
                    profile_color: u.profile_color.as_ref().and_then(ChatColor::parse),
                    ..User::empty().parse_status(u.status.as_ref())
                }
            }
            _ => User { id, ..User::empty() },
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
    use crate::types::{ChatColor, ChatPhoto};

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
            bot_forum_sidebar: false,
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
    fn a_user_with_no_optional_fields_parses_to_the_empty_user() {
        assert_eq!(parse(raw(7)), User { id: 7, first_name: "Neko".into(), ..User::empty() });
    }

    #[test]
    fn reads_contact_and_bot_flags() {
        type Set = fn(&mut types::User);
        type Get = fn(&User) -> bool;
        let cases: [(Set, Get); 13] = [
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
            (|u| u.bot_inline_geo = true, |u| u.inline_needs_location),
            (|u| u.bot_can_edit = true, |u| u.can_be_edited),
            (|u| u.bot_forum_sidebar = true, |u| u.has_topics_sidebar),
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

    fn username(name: &str, editable: bool, active: bool) -> base::Username {
        let u = types::Username { editable, active, deletable: false, expired: false, username: name.into() };
        base::Username::Username(u.into())
    }

    #[test]
    fn username_falls_back_to_the_first_active_collectible_username() {
        let mut user = raw(7);
        user.usernames = Some(vec![username("first", false, false), username("second", false, true)]);
        let parsed = parse(user.clone());
        assert_eq!(parsed.username.as_deref(), Some("second"));
        assert_eq!(parsed.usernames.len(), 2);
        assert!(!parsed.usernames[0].is_editable);
        user.username = Some("own".into());
        assert_eq!(parse(user).username.as_deref(), Some("own"));
    }

    fn photo(has_video: bool, personal: bool) -> base::UserProfilePhoto {
        let p = types::UserProfilePhoto { has_video, personal, photo_id: 123, stripped_thumb: None, dc_id: 4 };
        base::UserProfilePhoto::UserProfilePhoto(p.into())
    }

    #[test]
    fn a_photo_needs_an_access_hash() {
        let mut user = raw(7);
        user.min = true;
        user.photo = Some(photo(true, false));
        let parsed = parse(user.clone());
        assert_eq!(parsed.photo, None);
        assert_eq!(parsed.dc_id, Some(4));
        user.access_hash = Some(99);
        let parsed = parse(user);
        assert_eq!(parsed.photo, Some(ChatPhoto::new(7, 99, 123, 4, true, false)));
        assert_eq!(parsed.dc_id, Some(4));
    }

    #[test]
    fn an_empty_profile_photo_is_none() {
        let mut user = raw(7);
        user.access_hash = Some(99);
        user.photo = Some(base::UserProfilePhoto::UserProfilePhotoEmpty);
        let parsed = parse(user);
        assert_eq!((parsed.photo, parsed.dc_id), (None, None));
    }

    #[test]
    fn reads_reply_and_profile_colors() {
        let mut user = raw(7);
        user.color =
            Some(base::PeerColor::PeerColor(types::PeerColor { color: Some(3), background_emoji_id: None }.into()));
        user.profile_color =
            Some(base::PeerColor::InputPeerColorCollectible(types::InputPeerColorCollectible { collectible_id: 1 }));
        let parsed = parse(user);
        assert_eq!(parsed.reply_color, Some(ChatColor { color: Some(3), background_emoji_id: None }));
        assert_eq!(parsed.profile_color, None);
    }
}
