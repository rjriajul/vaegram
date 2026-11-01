use crate::dispatcher::Entities;
use crate::enums::ProfileTab;
use crate::raw::{base, types};
use crate::types::{BotVerification, Chat, ChatReactions};
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
}

impl ChatFullInfo {
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
