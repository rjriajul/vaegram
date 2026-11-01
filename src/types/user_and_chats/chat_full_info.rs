use crate::dispatcher::Entities;
use crate::enums::ProfileTab;
use crate::raw::{base, types};
use crate::types::{
    AcceptedGiftTypes, Birthday, BotVerification, BusinessAwayMessage, BusinessGreetingMessage, BusinessIntro,
    BusinessLocation, BusinessOpeningHours, Chat, ChatPhoto, ChatPrivileges, ChatReactions, FormattedText, UserRating,
};
use crate::utils::{get_channel_id, get_peer};

/// What only the full profile of a chat tells; `get_chat` fills it. Each group of fields says which chats have it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatFullInfo {
    // Every chat.
    pub pinned_message_id: Option<i32>,
    pub folder_id: Option<i32>,
    pub message_auto_delete_time: Option<i32>,
    /// The emoji of the chat theme.
    pub theme: Option<String>,
    pub can_schedule_messages: bool,
    pub is_translations_disabled: bool,

    // Users, bots and channels.
    pub is_blocked: bool,
    pub is_pinned_stories_available: bool,
    pub can_view_revenue: bool,
    pub gift_count: Option<i32>,
    pub main_profile_tab: Option<ProfileTab>,
    pub bot_verification: Option<BotVerification>,

    // Groups and channels.
    /// The primary invite link, for admins.
    pub invite_link: Option<String>,
    pub join_requests_count: Option<i32>,
    /// `None` when not sent; a default value when no reactions are allowed.
    pub available_reactions: Option<ChatReactions>,
    pub reactions_limit: Option<i32>,
    pub can_set_username: bool,
    pub has_welcome_messages: bool,
    pub admins_count: Option<i32>,
    pub kicked_count: Option<i32>,

    // Supergroups and channels.
    /// The discussion group of a channel, or the channel of a discussion group.
    pub linked_chat: Option<Chat>,
    /// Who your messages are sent as by default.
    pub send_as_chat: Option<Chat>,
    pub slow_mode_delay: Option<i32>,
    pub slowmode_next_send_date: Option<i32>,
    pub sticker_set_name: Option<String>,
    pub custom_emoji_sticker_set_name: Option<String>,
    pub banned_count: Option<i32>,
    pub boosts_applied: Option<i32>,
    /// How many boosts lift the restrictions on you.
    pub unrestrict_boost_count: Option<i32>,
    /// The basic group this supergroup was upgraded from.
    pub migrated_from_chat_id: Option<i64>,
    pub migrated_from_max_message_id: Option<i32>,
    pub can_view_participants: bool,
    pub can_set_sticker_set: bool,
    /// New members see the messages sent before they joined.
    pub has_visible_history: bool,
    pub can_view_stats: bool,
    pub can_delete_channel: bool,
    pub has_aggressive_anti_spam_enabled: bool,
    pub is_members_hidden: bool,
    pub view_forum_as_messages: bool,
    pub is_ads_restricted: bool,
    pub can_send_paid_media: bool,
    pub can_view_stars_revenue: bool,
    pub is_paid_reactions_available: bool,
    pub can_send_gift: bool,
    pub is_paid_messages_available: bool,

    // Users and bots.
    pub birthday: Option<Birthday>,
    pub personal_channel: Option<Chat>,
    /// The latest post of the personal channel.
    pub personal_channel_message_id: Option<i32>,
    /// The photo you set for this contact; only you see it.
    pub personal_photo: Option<ChatPhoto>,
    /// The photo shown to those who may not see the main one.
    pub public_photo: Option<ChatPhoto>,
    pub common_chats_count: Option<i32>,
    /// The name shown on forwards when the user hides their account.
    pub private_forward_name: Option<String>,
    /// The rights a bot asks for when added to groups.
    pub chat_admin_rights: Option<ChatPrivileges>,
    /// The rights a bot asks for when added to channels.
    pub channel_admin_rights: Option<ChatPrivileges>,
    pub business_intro: Option<BusinessIntro>,
    pub business_location: Option<BusinessLocation>,
    pub business_opening_hours: Option<BusinessOpeningHours>,
    pub business_greeting_message: Option<BusinessGreetingMessage>,
    pub business_away_message: Option<BusinessAwayMessage>,
    pub rating: Option<UserRating>,
    /// Your rating once pending changes apply, at `pending_rating_date`.
    pub pending_rating: Option<UserRating>,
    pub pending_rating_date: Option<i32>,
    pub accepted_gift_types: Option<AcceptedGiftTypes>,
    /// Your private note about this contact.
    pub note: Option<FormattedText>,
    pub is_phone_calls_available: bool,
    pub is_phone_calls_private: bool,
    pub is_video_calls_available: bool,
    pub can_pin_message: bool,
    pub can_send_voice_messages: bool,
    /// The user can't see your stories.
    pub is_blocked_my_stories_from: bool,
    /// Messaging this user needs Premium.
    pub is_contact_require_premium: bool,
    pub is_read_dates_available: bool,
    pub is_ads_enabled: bool,
    pub bot_can_manage_emoji_status: bool,
    pub display_gifts_button: bool,
    /// The user's app is not an official or verified one.
    pub uses_unofficial_app: bool,
}

impl ChatFullInfo {
    pub(crate) fn parse_user(u: types::UserFull, entities: &Entities) -> ChatFullInfo {
        let id = u.id;
        let access_hash = match entities.users.get(&id) {
            Some(base::User::User(user)) => user.access_hash,
            _ => None,
        };
        // Downloading either photo goes through the user, so it needs the user's access hash.
        let photo = |photo: Option<base::Photo>, personal| match (access_hash, photo) {
            (Some(hash), Some(base::Photo::Photo(p))) => {
                Some(ChatPhoto::new(id, hash, p.id, p.dc_id, p.video_sizes.is_some(), personal))
            }
            _ => None,
        };
        ChatFullInfo {
            pinned_message_id: u.pinned_msg_id,
            folder_id: u.folder_id,
            message_auto_delete_time: u.ttl_period,
            theme: u.theme.and_then(|theme| match theme {
                base::ChatTheme::ChatTheme(t) => Some(t.emoticon),
                base::ChatTheme::ChatThemeUniqueGift(_) => None,
            }),
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
            ..ChatFullInfo::default()
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
            ..ChatFullInfo::default()
        }
    }

    pub(crate) fn parse_channel(c: types::ChannelFull, entities: &Entities) -> ChatFullInfo {
        ChatFullInfo {
            pinned_message_id: c.pinned_msg_id,
            folder_id: c.folder_id,
            message_auto_delete_time: c.ttl_period,
            theme: c.theme_emoticon,
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
            ..ChatFullInfo::default()
        }
    }

    pub(crate) fn parse_community(c: &types::CommunityFull) -> ChatFullInfo {
        ChatFullInfo {
            join_requests_count: c.peer_link_requests_pending,
            admins_count: c.admins_count,
            kicked_count: c.kicked_count,
            ..ChatFullInfo::default()
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
    use super::linked_chat;
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
}
