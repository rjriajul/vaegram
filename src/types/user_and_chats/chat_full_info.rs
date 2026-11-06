use crate::dispatcher::Entities;
use crate::enums::ProfileTab;
use crate::raw::{base, types};
use crate::types::{
    AcceptedGiftTypes, Birthday, BotVerification, BusinessAwayMessage, BusinessGreetingMessage, BusinessIntro,
    BusinessLocation, BusinessOpeningHours, Chat, ChatBackground, ChatLocation, ChatPhoto, ChatPrivileges,
    ChatReactions, FormattedText, ShareTheme, UserRating,
};
use crate::utils::{get_channel_id, get_peer};

/// Details from the full profile of a chat, which only `get_chat` fetches.
///
/// Users and bots, basic groups, and supergroups and channels each fill a different set of fields. Fields a
/// chat kind doesn't have stay `None` or `false`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ChatFullInfo {
    // Every chat.
    /// Id of the latest pinned message. `None` if nothing is pinned.
    pub pinned_message_id: Option<i32>,
    /// Id of the chat folder the chat is in. `None` if it isn't in one.
    pub folder_id: Option<i32>,
    /// The background of this chat for you.
    pub chat_background: Option<ChatBackground>,
    /// `true` if your background in this private chat replaces the one the other side set.
    pub is_wallpaper_overridden: bool,
    /// Seconds after which new messages are deleted. `None` if auto-delete is off.
    pub message_auto_delete_time: Option<i32>,
    /// The emoji of the chat theme. `None` if no theme is set or a user's theme comes from a unique gift.
    pub theme: Option<String>,
    /// The theme of this chat's share card. `None` for the default one.
    pub share_theme: Option<ShareTheme>,
    /// `true` if the chat has scheduled messages.
    pub can_schedule_messages: bool,
    /// `true` if automatic translation is turned off for the chat.
    pub is_translations_disabled: bool,

    // Users, bots and channels.
    /// `true` if you blocked this chat.
    pub is_blocked: bool,
    /// `true` if there are pinned stories to show.
    pub is_pinned_stories_available: bool,
    /// `true` if you can see the revenue statistics.
    pub can_view_revenue: bool,
    /// Number of gifts shown on the profile.
    pub gift_count: Option<i32>,
    /// The tab the profile opens on.
    pub main_profile_tab: Option<ProfileTab>,
    /// The verification badge a bot gave the chat.
    pub bot_verification: Option<BotVerification>,

    // Groups and channels.
    /// The primary invite link. Only admins get it.
    pub invite_link: Option<String>,
    /// Number of join requests waiting for approval.
    pub join_requests_count: Option<i32>,
    /// Reactions members can use. Holds no reactions, with `all_are_enabled` `false`, when reactions are off.
    pub available_reactions: Option<ChatReactions>,
    /// Most reactions one user can put on a message.
    pub reactions_limit: Option<i32>,
    /// `true` if you can set a public username for the chat.
    pub can_set_username: bool,
    /// `true` if the chat shows welcome messages to new members.
    pub has_welcome_messages: bool,
    /// Number of administrators.
    pub admins_count: Option<i32>,
    /// Number of users removed from the chat.
    pub kicked_count: Option<i32>,

    // Supergroups and channels.
    /// The discussion group of a channel, or the channel of a discussion group.
    pub linked_chat: Option<Chat>,
    /// Who your messages are sent as by default.
    pub send_as_chat: Option<Chat>,
    /// Seconds a member has to wait between messages. `None` if slow mode is off.
    pub slow_mode_delay: Option<i32>,
    /// Unix time you can send again under slow mode.
    pub slowmode_next_send_date: Option<i32>,
    /// Short name of the group's sticker set.
    pub sticker_set_name: Option<String>,
    /// Short name of the group's custom emoji set.
    pub custom_emoji_sticker_set_name: Option<String>,
    /// Number of banned users.
    pub banned_count: Option<i32>,
    /// Number of boosts you applied to the chat.
    pub boosts_applied: Option<i32>,
    /// How many boosts lift the restrictions on you.
    pub unrestrict_boost_count: Option<i32>,
    /// Id of the basic group this supergroup was upgraded from.
    pub migrated_from_chat_id: Option<i64>,
    /// Id of the last message in the basic group before the upgrade.
    pub migrated_from_max_message_id: Option<i32>,
    /// `true` if you can list the members.
    pub can_view_participants: bool,
    /// `true` if you can set the group's sticker set.
    pub can_set_sticker_set: bool,
    /// `true` if new members see the messages sent before they joined.
    pub has_visible_history: bool,
    /// `true` if you can see the chat's statistics.
    pub can_view_stats: bool,
    /// Where the group is, for location-based groups.
    pub location: Option<ChatLocation>,
    /// `true` if you can set the group's location.
    pub can_set_location: bool,
    /// Messages up to this id were left out of the history available to you.
    pub available_min_id: Option<i32>,
    /// The data center that holds the statistics of the chat.
    pub stats_dc_id: Option<i32>,
    /// `true` if you can delete the channel.
    pub can_delete_channel: bool,
    /// `true` if aggressive anti-spam is on.
    pub has_aggressive_anti_spam_enabled: bool,
    /// `true` if the member list is hidden.
    pub is_members_hidden: bool,
    /// `true` if you view the forum as a single message list instead of topics.
    pub view_forum_as_messages: bool,
    /// `true` if sponsored messages are turned off in the chat.
    pub is_ads_restricted: bool,
    /// `true` if you can send paid media to the channel.
    pub can_send_paid_media: bool,
    /// `true` if you can see the Stars revenue.
    pub can_view_stars_revenue: bool,
    /// `true` if paid reactions are enabled.
    pub is_paid_reactions_available: bool,
    /// `true` if gifts can be sent to the chat.
    pub can_send_gift: bool,
    /// `true` if the chat accepts paid messages.
    pub is_paid_messages_available: bool,

    // Users and bots.
    /// The user's birthday. `None` if hidden or not set.
    pub birthday: Option<Birthday>,
    /// The channel the user shows on their profile.
    pub personal_channel: Option<Chat>,
    /// The latest post of the personal channel.
    pub personal_channel_message_id: Option<i32>,
    /// The photo you set for this contact. Only you see it.
    pub personal_photo: Option<ChatPhoto>,
    /// The photo shown to those who may not see the main one.
    pub public_photo: Option<ChatPhoto>,
    /// Number of groups you share with the user. Set for every user.
    pub common_chats_count: Option<i32>,
    /// The name shown on forwards when the user hides their account.
    pub private_forward_name: Option<String>,
    /// The rights a bot asks for when added to groups.
    pub chat_admin_rights: Option<ChatPrivileges>,
    /// The rights a bot asks for when added to channels.
    pub channel_admin_rights: Option<ChatPrivileges>,
    /// The business account's intro.
    pub business_intro: Option<BusinessIntro>,
    /// The business account's location.
    pub business_location: Option<BusinessLocation>,
    /// The business account's opening hours.
    pub business_opening_hours: Option<BusinessOpeningHours>,
    /// The business account's automatic greeting.
    pub business_greeting_message: Option<BusinessGreetingMessage>,
    /// The business account's automatic away reply.
    pub business_away_message: Option<BusinessAwayMessage>,
    /// The user's Stars rating.
    pub rating: Option<UserRating>,
    /// Your rating once pending changes apply, at `pending_rating_date`.
    pub pending_rating: Option<UserRating>,
    /// Unix time `pending_rating` applies.
    pub pending_rating_date: Option<i32>,
    /// The kinds of gifts the user accepts.
    pub accepted_gift_types: Option<AcceptedGiftTypes>,
    /// Your private note about this contact.
    pub note: Option<FormattedText>,
    /// `true` if you can call the user.
    pub is_phone_calls_available: bool,
    /// `true` if the user's privacy settings keep you from calling them.
    pub is_phone_calls_private: bool,
    /// `true` if you can video call the user.
    pub is_video_calls_available: bool,
    /// `true` if you can pin messages in the private chat.
    pub can_pin_message: bool,
    /// `false` if the user doesn't accept voice and video messages.
    pub can_send_voice_messages: bool,
    /// `true` if the user can't see your stories.
    pub is_blocked_my_stories_from: bool,
    /// `true` if messaging this user needs Telegram Premium.
    pub is_contact_require_premium: bool,
    /// `false` if the user hides when they read messages.
    pub is_read_dates_available: bool,
    /// `true` if you turned sponsored messages back on. Only set on your own profile.
    pub is_ads_enabled: bool,
    /// `true` if the bot may manage the user's emoji status.
    pub bot_can_manage_emoji_status: bool,
    /// `true` if the user shows a gift button in the message input field.
    pub display_gifts_button: bool,
    /// `true` if the user's app is not an official or verified one.
    pub uses_unofficial_app: bool,
}

impl ChatFullInfo {
    /// Every field zero, empty or `None`: the base the parsers and tests fill in.
    pub(crate) fn empty() -> Self {
        ChatFullInfo {
            pinned_message_id: None,
            folder_id: None,
            chat_background: None,
            is_wallpaper_overridden: false,
            message_auto_delete_time: None,
            theme: None,
            share_theme: None,
            can_schedule_messages: false,
            is_translations_disabled: false,
            is_blocked: false,
            is_pinned_stories_available: false,
            can_view_revenue: false,
            gift_count: None,
            main_profile_tab: None,
            bot_verification: None,
            invite_link: None,
            join_requests_count: None,
            available_reactions: None,
            reactions_limit: None,
            can_set_username: false,
            has_welcome_messages: false,
            admins_count: None,
            kicked_count: None,
            linked_chat: None,
            send_as_chat: None,
            slow_mode_delay: None,
            slowmode_next_send_date: None,
            sticker_set_name: None,
            custom_emoji_sticker_set_name: None,
            banned_count: None,
            boosts_applied: None,
            unrestrict_boost_count: None,
            migrated_from_chat_id: None,
            migrated_from_max_message_id: None,
            can_view_participants: false,
            can_set_sticker_set: false,
            has_visible_history: false,
            can_view_stats: false,
            location: None,
            can_set_location: false,
            available_min_id: None,
            stats_dc_id: None,
            can_delete_channel: false,
            has_aggressive_anti_spam_enabled: false,
            is_members_hidden: false,
            view_forum_as_messages: false,
            is_ads_restricted: false,
            can_send_paid_media: false,
            can_view_stars_revenue: false,
            is_paid_reactions_available: false,
            can_send_gift: false,
            is_paid_messages_available: false,
            birthday: None,
            personal_channel: None,
            personal_channel_message_id: None,
            personal_photo: None,
            public_photo: None,
            common_chats_count: None,
            private_forward_name: None,
            chat_admin_rights: None,
            channel_admin_rights: None,
            business_intro: None,
            business_location: None,
            business_opening_hours: None,
            business_greeting_message: None,
            business_away_message: None,
            rating: None,
            pending_rating: None,
            pending_rating_date: None,
            accepted_gift_types: None,
            note: None,
            is_phone_calls_available: false,
            is_phone_calls_private: false,
            is_video_calls_available: false,
            can_pin_message: false,
            can_send_voice_messages: false,
            is_blocked_my_stories_from: false,
            is_contact_require_premium: false,
            is_read_dates_available: false,
            is_ads_enabled: false,
            bot_can_manage_emoji_status: false,
            display_gifts_button: false,
            uses_unofficial_app: false,
        }
    }

    pub(crate) fn parse_user(u: types::UserFull, entities: &Entities) -> ChatFullInfo {
        let id = u.id;
        let access_hash = match entities.users.get(&id) {
            Some(base::User::User(user)) => user.access_hash,
            _ => None,
        };
        // Downloading either photo goes through the user, so it needs the user's access hash.
        let photo = |photo: Option<base::Photo>, personal| match (access_hash, photo) {
            (Some(hash), Some(base::Photo::Photo(p))) => {
                Some(ChatPhoto::new(id, hash, p.id, p.dc_id, p.video_sizes.is_some(), personal).with_full(&p))
            }
            _ => None,
        };
        ChatFullInfo {
            pinned_message_id: u.pinned_msg_id,
            folder_id: u.folder_id,
            chat_background: u.wallpaper.map(ChatBackground::parse),
            is_wallpaper_overridden: u.wallpaper_overridden,
            message_auto_delete_time: u.ttl_period,
            theme: u.theme.and_then(|theme| match theme {
                base::ChatTheme::ChatTheme(t) => Some(t.emoticon),
                base::ChatTheme::ChatThemeUniqueGift(_) => None,
            }),
            share_theme: u.share_theme.as_ref().and_then(|t| ShareTheme::parse(t, entities)),
            can_schedule_messages: u.has_scheduled,
            is_translations_disabled: u.translations_disabled,
            is_blocked: u.blocked,
            is_pinned_stories_available: u.stories_pinned_available,
            can_view_revenue: u.can_view_revenue,
            gift_count: u.stargifts_count,
            main_profile_tab: u.main_tab.as_ref().map(ProfileTab::parse),
            bot_verification: u.bot_verification.map(|v| BotVerification::parse(v, entities)),
            birthday: u.birthday.as_ref().map(Birthday::parse),
            personal_channel: u
                .personal_channel_id
                .map(|id| Chat::parse(&get_peer(get_channel_id(id)), entities, true)),
            personal_channel_message_id: u.personal_channel_message,
            personal_photo: photo(u.personal_photo, true),
            public_photo: photo(u.fallback_photo, false),
            common_chats_count: Some(u.common_chats_count),
            private_forward_name: u.private_forward_name,
            chat_admin_rights: u.bot_group_admin_rights.as_ref().map(ChatPrivileges::parse),
            channel_admin_rights: u.bot_broadcast_admin_rights.as_ref().map(ChatPrivileges::parse),
            business_intro: u.business_intro.map(BusinessIntro::parse),
            business_location: u.business_location.map(BusinessLocation::parse),
            business_opening_hours: u.business_work_hours.map(BusinessOpeningHours::parse),
            business_greeting_message: u.business_greeting_message.map(BusinessGreetingMessage::parse),
            business_away_message: u.business_away_message.map(BusinessAwayMessage::parse),
            rating: u.stars_rating.as_ref().map(UserRating::parse),
            pending_rating: u.stars_my_pending_rating.as_ref().map(UserRating::parse),
            pending_rating_date: u.stars_my_pending_rating_date,
            accepted_gift_types: u.disallowed_gifts.as_ref().map(AcceptedGiftTypes::parse),
            note: u.note.map(FormattedText::parse),
            is_phone_calls_available: u.phone_calls_available,
            is_phone_calls_private: u.phone_calls_private,
            is_video_calls_available: u.video_calls_available,
            can_pin_message: u.can_pin_message,
            can_send_voice_messages: !u.voice_messages_forbidden,
            is_blocked_my_stories_from: u.blocked_my_stories_from,
            is_contact_require_premium: u.contact_require_premium,
            is_read_dates_available: !u.read_dates_private,
            is_ads_enabled: u.sponsored_enabled,
            bot_can_manage_emoji_status: u.bot_can_manage_emoji_status,
            display_gifts_button: u.display_gifts_button,
            uses_unofficial_app: u.unofficial_security_risk,
            ..ChatFullInfo::empty()
        }
    }

    pub(crate) fn parse_chat(c: types::ChatFull) -> ChatFullInfo {
        ChatFullInfo {
            pinned_message_id: c.pinned_msg_id,
            folder_id: c.folder_id,
            message_auto_delete_time: c.ttl_period,
            theme: c.theme_emoticon,
            can_schedule_messages: c.has_scheduled,
            is_translations_disabled: c.translations_disabled,
            invite_link: c.exported_invite.and_then(invite_link),
            join_requests_count: c.requests_pending,
            available_reactions: c.available_reactions.as_ref().map(ChatReactions::parse),
            reactions_limit: c.reactions_limit,
            can_set_username: c.can_set_username,
            has_welcome_messages: c.has_welcome_messages,
            ..ChatFullInfo::empty()
        }
    }

    pub(crate) fn parse_channel(c: types::ChannelFull, entities: &Entities) -> ChatFullInfo {
        ChatFullInfo {
            pinned_message_id: c.pinned_msg_id,
            folder_id: c.folder_id,
            chat_background: c.wallpaper.map(ChatBackground::parse),
            message_auto_delete_time: c.ttl_period,
            theme: c.theme_emoticon,
            share_theme: c.share_theme.as_ref().and_then(|t| ShareTheme::parse(t, entities)),
            can_schedule_messages: c.has_scheduled,
            is_translations_disabled: c.translations_disabled,
            is_blocked: c.blocked,
            is_pinned_stories_available: c.stories_pinned_available,
            can_view_revenue: c.can_view_revenue,
            gift_count: c.stargifts_count,
            main_profile_tab: c.main_tab.as_ref().map(ProfileTab::parse),
            bot_verification: c.bot_verification.map(|v| BotVerification::parse(v, entities)),
            invite_link: c.exported_invite.and_then(invite_link),
            join_requests_count: c.requests_pending,
            available_reactions: c.available_reactions.as_ref().map(ChatReactions::parse),
            reactions_limit: c.reactions_limit,
            can_set_username: c.can_set_username,
            has_welcome_messages: c.has_welcome_messages,
            admins_count: c.admins_count,
            kicked_count: c.kicked_count,
            linked_chat: c.linked_chat_id.map(|id| linked_chat(c.id, id, entities)),
            send_as_chat: c.default_send_as.map(|peer| Chat::parse(&peer, entities, false)),
            slow_mode_delay: c.slowmode_seconds,
            slowmode_next_send_date: c.slowmode_next_send_date,
            sticker_set_name: c.stickerset.map(short_name),
            custom_emoji_sticker_set_name: c.emojiset.map(short_name),
            banned_count: c.banned_count,
            boosts_applied: c.boosts_applied,
            unrestrict_boost_count: c.boosts_unrestrict,
            // Basic groups have positive raw ids; the Bot API id is the negation.
            migrated_from_chat_id: c.migrated_from_chat_id.map(|id| -id),
            migrated_from_max_message_id: c.migrated_from_max_id,
            can_view_participants: c.can_view_participants,
            can_set_sticker_set: c.can_set_stickers,
            has_visible_history: !c.hidden_prehistory,
            can_view_stats: c.can_view_stats,
            location: c.location.and_then(ChatLocation::parse),
            can_set_location: c.can_set_location,
            available_min_id: c.available_min_id,
            stats_dc_id: c.stats_dc,
            can_delete_channel: c.can_delete_channel,
            has_aggressive_anti_spam_enabled: c.antispam,
            is_members_hidden: c.participants_hidden,
            view_forum_as_messages: c.view_forum_as_messages,
            is_ads_restricted: c.restricted_sponsored,
            can_send_paid_media: c.paid_media_allowed,
            can_view_stars_revenue: c.can_view_stars_revenue,
            is_paid_reactions_available: c.paid_reactions_available,
            can_send_gift: c.stargifts_available,
            is_paid_messages_available: c.paid_messages_available,
            ..ChatFullInfo::empty()
        }
    }

    pub(crate) fn parse_community(c: &types::CommunityFull) -> ChatFullInfo {
        ChatFullInfo {
            join_requests_count: c.peer_link_requests_pending,
            admins_count: c.admins_count,
            kicked_count: c.kicked_count,
            ..ChatFullInfo::empty()
        }
    }
}

/// A supergroup links to its broadcast channel, a channel to its discussion supergroup.
fn linked_chat(channel_id: i64, linked_id: i64, entities: &Entities) -> Chat {
    let is_megagroup = match entities.chats.get(&get_channel_id(channel_id)) {
        Some(base::Chat::Channel(c)) => c.megagroup,
        Some(base::Chat::ChannelForbidden(c)) => c.megagroup,
        _ => false,
    };
    Chat::parse(&get_peer(get_channel_id(linked_id)), entities, is_megagroup)
}

fn invite_link(invite: base::ExportedChatInvite) -> Option<String> {
    match invite {
        base::ExportedChatInvite::ChatInviteExported(i) => Some(i.link),
        base::ExportedChatInvite::ChatInvitePublicJoinRequests => None,
    }
}

fn short_name(set: base::StickerSet) -> String {
    let base::StickerSet::StickerSet(s) = set;
    s.short_name
}

#[cfg(test)]
mod tests {
    use super::{ChatFullInfo, ShareTheme, linked_chat};
    use crate::dispatcher::Entities;
    use crate::enums::ChatType;
    use crate::raw::{Serializable, base, types};
    use crate::utils::get_channel_id;

    fn raw_channel(id: i64, megagroup: bool) -> base::Chat {
        let mut buf = Vec::new();
        0u32.serialize(&mut buf);
        0u32.serialize(&mut buf);
        id.serialize(&mut buf);
        "c".to_owned().serialize(&mut buf);
        base::ChatPhoto::ChatPhotoEmpty.serialize(&mut buf);
        0i32.serialize(&mut buf);
        let mut channel = types::Channel::deserialize_bare(&mut &buf[..]).unwrap();
        channel.megagroup = megagroup;
        base::Chat::Channel(Box::new(channel))
    }

    #[test]
    fn linked_chat_kind_without_its_entity() {
        let mut entities = Entities::default();
        entities.chats.insert(get_channel_id(1), raw_channel(1, true));
        entities.chats.insert(get_channel_id(2), raw_channel(2, false));
        assert_eq!(linked_chat(1, 10, &entities).kind, ChatType::Channel);
        assert_eq!(linked_chat(2, 20, &entities).kind, ChatType::Supergroup);
        assert_eq!(linked_chat(3, 30, &entities).kind, ChatType::Supergroup);
    }

    #[test]
    fn channel_full_reads_location_background_and_stats_dc() {
        let mut buf = Vec::new();
        for word in [0u32, 0] {
            word.serialize(&mut buf);
        }
        7i64.serialize(&mut buf);
        "about".to_owned().serialize(&mut buf);
        for n in [0i32, 0, 0] {
            n.serialize(&mut buf);
        }
        base::Photo::PhotoEmpty(types::PhotoEmpty { id: 0 }).serialize(&mut buf);
        let settings = types::PeerNotifySettings::deserialize_bare(&mut &0u32.to_le_bytes()[..]).unwrap();
        base::PeerNotifySettings::PeerNotifySettings(settings.into()).serialize(&mut buf);
        Vec::<i64>::new().serialize(&mut buf);
        0i32.serialize(&mut buf);
        let point = types::GeoPoint { long: 13.4, lat: 52.5, access_hash: 1, accuracy_radius: None };
        let location = types::ChannelLocation {
            geo_point: base::GeoPoint::GeoPoint(point.into()),
            address: "1 Cat Street".into(),
        };
        let wallpaper = types::WallPaperNoFile { id: 3, default: false, dark: true, settings: None };
        let full = types::ChannelFull {
            can_set_location: true,
            location: Some(base::ChannelLocation::ChannelLocation(location.into())),
            wallpaper: Some(base::WallPaper::WallPaperNoFile(wallpaper.into())),
            available_min_id: Some(12),
            stats_dc: Some(4),
            share_theme: Some(backdrop_theme()),
            ..types::ChannelFull::deserialize_bare(&mut &buf[..]).unwrap()
        };
        let info = ChatFullInfo::parse_channel(full, &Entities::default());
        let location = info.location.expect("the location");
        assert_eq!((location.address.as_str(), location.location.latitude), ("1 Cat Street", 52.5));
        assert!(info.can_set_location && !info.is_wallpaper_overridden);
        assert_eq!(info.chat_background.map(|b| (b.id, b.is_dark)), Some((3, true)));
        assert_eq!((info.available_min_id, info.stats_dc_id), (Some(12), Some(4)));
        assert_eq!(info.share_theme.map(|t| t.backdrop_id), Some(Some(3)));
    }

    #[test]
    fn user_full_reads_wallpaper_and_override() {
        use crate::raw::Identifiable;
        let mut buf = Vec::new();
        for word in [0u32, 0] {
            word.serialize(&mut buf);
        }
        7i64.serialize(&mut buf);
        types::PeerSettings::CONSTRUCTOR_ID.serialize(&mut buf);
        0u32.serialize(&mut buf);
        let settings = types::PeerNotifySettings::deserialize_bare(&mut &0u32.to_le_bytes()[..]).unwrap();
        base::PeerNotifySettings::PeerNotifySettings(settings.into()).serialize(&mut buf);
        3i32.serialize(&mut buf);
        let wallpaper = types::WallPaperNoFile { id: 9, default: false, dark: true, settings: None };
        let full = types::UserFull {
            wallpaper: Some(base::WallPaper::WallPaperNoFile(wallpaper.into())),
            wallpaper_overridden: true,
            share_theme: Some(backdrop_theme()),
            ..types::UserFull::deserialize_bare(&mut &buf[..]).unwrap()
        };
        let info = ChatFullInfo::parse_user(full, &Entities::default());
        assert!(info.is_wallpaper_overridden);
        assert_eq!(info.share_theme.map(|t| t.backdrop_id), Some(Some(3)));
        assert_eq!(info.chat_background.map(|b| (b.id, b.is_dark)), Some((9, true)));

        let plain = types::UserFull::deserialize_bare(&mut &buf[..]).unwrap();
        let info = ChatFullInfo::parse_user(plain, &Entities::default());
        assert!(!info.is_wallpaper_overridden && info.chat_background.is_none() && info.share_theme.is_none());
    }

    fn backdrop_theme() -> base::ShareTheme {
        base::ShareTheme::ShareThemeEmojiBackdrop(Box::new(types::ShareThemeEmojiBackdrop {
            backdrop_id: Some(3),
            emojis: Some(vec![base::Reaction::ReactionEmoji(Box::new(types::ReactionEmoji {
                emoticon: "🐱".into()
            }))]),
        }))
    }

    #[test]
    fn full_info_reads_share_theme() {
        let entities = Entities::default();
        let read = ShareTheme::parse(&backdrop_theme(), &entities).expect("a backdrop theme");
        assert_eq!(read.backdrop_id, Some(3));
        assert_eq!(read.emojis, [crate::types::ReactionType::Emoji("🐱".into())]);
        assert!(read.gift.is_none());
        assert_eq!(ShareTheme::parse(&base::ShareTheme::ShareThemeDefault, &entities), None);
    }
}
