use crate::dispatcher::Entities;
use crate::enums::ChatType;
use crate::raw::base;
use crate::types::{
    ChatColor, ChatFullInfo, ChatPermissions, ChatPhoto, ChatPrivileges, Community, EmojiStatus, RestrictionReason,
    User, Username,
};
use crate::utils::{get_channel_id, get_peer, get_peer_id};

/// A user, bot, group, supergroup or channel.
///
/// Fields that Telegram doesn't send for a chat kind, or that the update didn't carry, stay `None` or `false`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chat {
    /// Chat id. Users are positive, basic groups negative and channels in the form `-1001234567890`.
    pub id: i64,
    /// Kind of chat: private, bot, group, supergroup or channel.
    pub kind: ChatType,
    /// Title of a group or channel. `None` for private chats.
    pub title: Option<String>,
    /// The first active username when the chat has several.
    pub username: Option<String>,
    /// First name of a user or bot. `None` for groups and channels.
    pub first_name: Option<String>,
    /// Last name of a user. `None` if unset or for groups and channels.
    pub last_name: Option<String>,
    /// Bio of a user. Only `get_chat` fills it.
    pub bio: Option<String>,
    /// Description of a group or channel. Only `get_chat` fills it.
    pub description: Option<String>,
    /// The chat's primary invite link, from `get_chat`.
    pub invite_link: Option<String>,
    /// Number of members. `None` for users, for chats you're banned from, and for supergroups and channels when
    /// Telegram omits it.
    pub members_count: Option<i32>,
    /// The bot answering join request queries. Only admins get it, and only from `get_chat`.
    pub guard_bot: Option<User>,
    /// The community the chat or bot belongs to. Only `get_chat` fills it.
    pub community: Option<Community>,
    /// `true` for the direct messages chat of a channel.
    pub is_direct_messages: bool,
    /// The channel a direct messages chat belongs to. `None` for other chats.
    pub parent_chat: Option<Box<Chat>>,
    /// `true` if the supergroup has topics enabled.
    pub is_forum: bool,
    /// Whether you are an admin of this group or channel.
    pub is_admin: bool,
    /// `true` if Telegram verified the account or chat.
    pub is_verified: bool,
    /// `true` if Telegram flagged the account or chat as a scam.
    pub is_scam: bool,
    /// `true` if Telegram flagged the account or chat as impersonating someone.
    pub is_fake: bool,
    /// `true` for Telegram support accounts.
    pub is_support: bool,
    /// `true` if access is restricted for reasons listed in `restrictions`.
    pub is_restricted: bool,
    /// Why and where the chat is restricted. Empty if it isn't.
    pub restrictions: Vec<RestrictionReason>,
    /// `true` if you hid the stories of this user or channel.
    pub is_stories_hidden: bool,
    /// `true` if stories can't be shown for this user or channel.
    pub is_stories_unavailable: bool,
    /// The icon of the bot that verified this user or channel.
    pub bot_verification_icon_custom_emoji_id: Option<i64>,
    /// All usernames, active or not. Empty if there are none.
    pub usernames: Vec<Username>,
    /// Profile photo. `None` if the chat has none or Telegram sent no access hash for it.
    pub photo: Option<ChatPhoto>,
    /// The data center of the chat photo.
    pub dc_id: Option<i32>,
    /// Accent color of replies and the name. `None` for basic groups and when unset.
    pub reply_color: Option<ChatColor>,
    /// Accent color of the profile page. `None` for basic groups and when unset.
    pub profile_color: Option<ChatColor>,
    /// Emoji shown next to the name. `None` if unset.
    pub emoji_status: Option<EmojiStatus>,
    /// What a message to this user or chat costs you.
    pub paid_message_star_count: Option<i64>,
    /// `true` if you created this group or channel.
    pub is_creator: bool,
    /// Your own admin rights here.
    pub privileges: Option<ChatPrivileges>,
    /// `true` if the basic group was upgraded and is no longer in use.
    pub is_deactivated: bool,
    /// `true` if a call is going on in the group or channel.
    pub is_call_active: bool,
    /// `true` if at least one participant is in the call.
    pub is_call_not_empty: bool,
    /// The default rights of members.
    pub permissions: Option<ChatPermissions>,
    /// `true` if forwarding and saving content from the chat is off.
    pub has_protected_content: bool,
    /// You were removed from this group or channel.
    pub is_banned: bool,
    /// Unix time your ban from the channel ends. `None` if you aren't banned or the ban has no end date.
    pub banned_until_date: Option<i32>,
    /// `true` if channel posts show the profile of the admin who wrote them.
    pub show_message_sender_name: bool,
    /// `true` if channel posts carry the signature of their author.
    pub sign_messages: bool,
    /// Boost level of the supergroup or channel. `None` for other chats or when Telegram omits it.
    pub level: Option<i32>,
    /// `true` if people must join the supergroup before they can send messages.
    pub join_to_send_messages: bool,
    /// `true` if joining needs an admin's approval.
    pub join_by_request: bool,
    /// Unix time your paid subscription to the channel ends. `None` if you have none.
    pub subscription_until_date: Option<i32>,
    /// `true` if the channel translates its posts automatically.
    pub has_automatic_translation: bool,
    /// `true` if the forum shows its topics as tabs.
    pub has_forum_tabs: bool,
    /// `true` if the channel accepts direct messages.
    pub has_direct_messages_group: bool,
    /// Id of the direct messages chat of this channel.
    pub direct_messages_chat_id: Option<i64>,
    /// Details from the full profile. Only `get_chat` fills it.
    pub full_info: Option<Box<ChatFullInfo>>,
}

impl Chat {
    pub(crate) fn parse(peer: &base::Peer, entities: &Entities, is_post: bool) -> Chat {
        let id = get_peer_id(peer);
        let mut chat = Chat {
            id,
            kind: Chat::kind_of(peer, entities, is_post),
            title: None,
            username: None,
            first_name: None,
            last_name: None,
            bio: None,
            description: None,
            invite_link: None,
            members_count: None,
            guard_bot: None,
            community: None,
            is_direct_messages: false,
            parent_chat: None,
            is_forum: false,
            is_admin: false,
            is_verified: false,
            is_scam: false,
            is_fake: false,
            is_support: false,
            is_restricted: false,
            restrictions: Vec::new(),
            is_stories_hidden: false,
            is_stories_unavailable: false,
            bot_verification_icon_custom_emoji_id: None,
            usernames: Vec::new(),
            photo: None,
            dc_id: None,
            reply_color: None,
            profile_color: None,
            emoji_status: None,
            paid_message_star_count: None,
            is_creator: false,
            privileges: None,
            is_deactivated: false,
            is_call_active: false,
            is_call_not_empty: false,
            permissions: None,
            has_protected_content: false,
            is_banned: false,
            banned_until_date: None,
            show_message_sender_name: false,
            sign_messages: false,
            level: None,
            join_to_send_messages: false,
            join_by_request: false,
            subscription_until_date: None,
            has_automatic_translation: false,
            has_forum_tabs: false,
            has_direct_messages_group: false,
            direct_messages_chat_id: None,
            full_info: None,
        };
        match peer {
            base::Peer::PeerUser(_) => {
                if let Some(base::User::User(u)) = entities.users.get(&id) {
                    chat.first_name = u.first_name.clone();
                    chat.last_name = u.last_name.clone();
                    chat.is_verified = u.verified;
                    chat.is_scam = u.scam;
                    chat.is_fake = u.fake;
                    chat.is_support = u.support;
                    chat.is_restricted = u.restricted;
                    chat.restrictions = restrictions(&u.restriction_reason);
                    chat.is_stories_hidden = u.stories_hidden;
                    chat.is_stories_unavailable = u.stories_unavailable;
                    chat.bot_verification_icon_custom_emoji_id = u.bot_verification_icon;
                    chat.usernames = usernames(&u.usernames);
                    chat.username = username(&u.username, &chat.usernames);
                    if let Some(base::UserProfilePhoto::UserProfilePhoto(p)) = &u.photo {
                        chat.dc_id = Some(p.dc_id);
                        chat.photo = u
                            .access_hash
                            .map(|hash| ChatPhoto::new(id, hash, p.photo_id, p.dc_id, p.has_video, p.personal));
                    }
                    chat.reply_color = u.color.as_ref().and_then(ChatColor::parse);
                    chat.profile_color = u.profile_color.as_ref().and_then(ChatColor::parse);
                    chat.emoji_status = u.emoji_status.as_ref().and_then(EmojiStatus::parse);
                    chat.paid_message_star_count = u.send_paid_messages_stars;
                }
            }
            base::Peer::PeerChat(_) => match entities.chats.get(&id) {
                Some(base::Chat::Chat(c)) => {
                    chat.title = Some(c.title.clone());
                    chat.is_admin = c.admin_rights.is_some();
                    chat.is_creator = c.creator;
                    chat.privileges = c.admin_rights.as_ref().map(ChatPrivileges::parse);
                    chat.is_deactivated = c.deactivated;
                    chat.is_call_active = c.call_active;
                    chat.is_call_not_empty = c.call_not_empty;
                    // Basic groups need no access hash to download their photo.
                    (chat.photo, chat.dc_id) = photo(id, Some(0), &c.photo);
                    chat.permissions = c.default_banned_rights.as_ref().map(ChatPermissions::parse);
                    chat.members_count = Some(c.participants_count);
                    chat.has_protected_content = c.noforwards;
                }
                Some(base::Chat::ChatForbidden(c)) => {
                    chat.title = Some(c.title.clone());
                    chat.is_banned = true;
                }
                _ => {}
            },
            base::Peer::PeerChannel(_) => match entities.chats.get(&id) {
                Some(base::Chat::Channel(c)) => {
                    chat.title = Some(c.title.clone());
                    chat.is_direct_messages = c.monoforum;
                    // A direct messages chat links back to its channel through the same field.
                    if c.monoforum {
                        chat.parent_chat = c
                            .linked_monoforum_id
                            .map(|parent| Box::new(Chat::parse(&get_peer(get_channel_id(parent)), entities, true)));
                    } else {
                        chat.direct_messages_chat_id = c.linked_monoforum_id.map(get_channel_id);
                    }
                    chat.is_forum = c.forum;
                    chat.is_admin = c.admin_rights.is_some();
                    chat.is_creator = c.creator;
                    chat.privileges = c.admin_rights.as_ref().map(ChatPrivileges::parse);
                    chat.is_call_active = c.call_active;
                    chat.is_call_not_empty = c.call_not_empty;
                    (chat.photo, chat.dc_id) = photo(id, c.access_hash, &c.photo);
                    chat.permissions = c.default_banned_rights.as_ref().map(ChatPermissions::parse);
                    chat.members_count = c.participants_count;
                    chat.has_protected_content = c.noforwards;
                    chat.is_verified = c.verified;
                    chat.is_scam = c.scam;
                    chat.is_fake = c.fake;
                    chat.is_restricted = c.restricted;
                    chat.restrictions = restrictions(&c.restriction_reason);
                    chat.is_stories_hidden = c.stories_hidden;
                    chat.is_stories_unavailable = c.stories_unavailable;
                    chat.bot_verification_icon_custom_emoji_id = c.bot_verification_icon;
                    chat.usernames = usernames(&c.usernames);
                    chat.username = username(&c.username, &chat.usernames);
                    chat.reply_color = c.color.as_ref().and_then(ChatColor::parse);
                    chat.profile_color = c.profile_color.as_ref().and_then(ChatColor::parse);
                    chat.emoji_status = c.emoji_status.as_ref().and_then(EmojiStatus::parse);
                    chat.paid_message_star_count = c.send_paid_messages_stars;
                    chat.show_message_sender_name = c.signature_profiles;
                    chat.sign_messages = c.signatures;
                    chat.level = c.level;
                    chat.join_to_send_messages = c.join_to_send;
                    chat.join_by_request = c.join_request;
                    chat.subscription_until_date = c.subscription_until_date;
                    chat.has_automatic_translation = c.autotranslation;
                    chat.has_forum_tabs = c.forum_tabs;
                    chat.has_direct_messages_group = c.broadcast_messages_allowed;
                }
                Some(base::Chat::ChannelForbidden(c)) => {
                    chat.title = Some(c.title.clone());
                    chat.is_direct_messages = c.monoforum;
                    chat.is_banned = true;
                    chat.banned_until_date = c.until_date;
                }
                _ => {}
            },
        }
        chat
    }

    pub(crate) fn kind_of(peer: &base::Peer, entities: &Entities, is_post: bool) -> ChatType {
        let id = get_peer_id(peer);
        match peer {
            base::Peer::PeerUser(_) => match entities.users.get(&id) {
                Some(base::User::User(u)) if u.bot => ChatType::Bot,
                _ => ChatType::Private,
            },
            base::Peer::PeerChat(_) => ChatType::Group,
            base::Peer::PeerChannel(_) => match entities.chats.get(&id) {
                Some(base::Chat::Channel(c)) if c.megagroup => ChatType::Supergroup,
                Some(base::Chat::Channel(_)) => ChatType::Channel,
                Some(base::Chat::ChannelForbidden(c)) if c.megagroup || !c.broadcast => ChatType::Supergroup,
                Some(base::Chat::ChannelForbidden(_)) => ChatType::Channel,
                _ if is_post => ChatType::Channel,
                _ => ChatType::Supergroup,
            },
        }
    }
}

fn restrictions(raw: &Option<Vec<base::RestrictionReason>>) -> Vec<RestrictionReason> {
    raw.iter().flatten().map(RestrictionReason::parse).collect()
}

fn usernames(raw: &Option<Vec<base::Username>>) -> Vec<Username> {
    raw.iter().flatten().map(Username::parse).collect()
}

/// Chats with several usernames send them only in the list.
fn username(single: &Option<String>, all: &[Username]) -> Option<String> {
    single.clone().or_else(|| all.iter().find(|u| u.active).map(|u| u.username.clone()))
}

/// The photo needs the peer's access hash to be downloaded, so a peer seen without one has none.
fn photo(id: i64, access_hash: Option<i64>, raw: &base::ChatPhoto) -> (Option<ChatPhoto>, Option<i32>) {
    match raw {
        base::ChatPhoto::ChatPhoto(p) => {
            let photo = access_hash.map(|hash| ChatPhoto::new(id, hash, p.photo_id, p.dc_id, p.has_video, false));
            (photo, Some(p.dc_id))
        }
        base::ChatPhoto::ChatPhotoEmpty => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::Chat;
    use crate::dispatcher::Entities;
    use crate::enums::ChatType;
    use crate::raw::{Serializable, base, types};
    use crate::types::{ChatColor, ChatPermissions, ChatPhoto, ChatPrivileges, EmojiStatus, RestrictionReason};
    use crate::utils::{get_channel_id, get_peer};

    fn raw_user(id: i64) -> types::User {
        let mut buf = Vec::new();
        for word in [0u32, 0] {
            word.serialize(&mut buf);
        }
        id.serialize(&mut buf);
        types::User::deserialize_bare(&mut &buf[..]).unwrap()
    }

    fn raw_channel(id: i64) -> types::Channel {
        let mut buf = Vec::new();
        0u32.serialize(&mut buf);
        0u32.serialize(&mut buf);
        id.serialize(&mut buf);
        "c".to_owned().serialize(&mut buf);
        base::ChatPhoto::ChatPhotoEmpty.serialize(&mut buf);
        0i32.serialize(&mut buf);
        types::Channel::deserialize_bare(&mut &buf[..]).unwrap()
    }

    fn raw_photo(photo_id: i64, dc_id: i32) -> base::ChatPhoto {
        let photo = types::ChatPhoto { has_video: false, photo_id, stripped_thumb: None, dc_id };
        base::ChatPhoto::ChatPhoto(photo.into())
    }

    fn username(name: &str, active: bool) -> base::Username {
        let u = types::Username { editable: false, active, deletable: false, expired: false, username: name.into() };
        base::Username::Username(u.into())
    }

    fn restriction() -> base::RestrictionReason {
        let r = types::RestrictionReason { platform: "ios".into(), reason: "porn".into(), text: "hidden".into() };
        base::RestrictionReason::RestrictionReason(r.into())
    }

    fn color(color: i32) -> base::PeerColor {
        base::PeerColor::PeerColor(types::PeerColor { color: Some(color), background_emoji_id: None }.into())
    }

    fn parse_chat(id: i64, chat: base::Chat) -> Chat {
        let entities = Entities { users: Default::default(), chats: [(id, chat)].into() };
        Chat::parse(&get_peer(id), &entities, false)
    }

    #[test]
    fn users_carry_photo_usernames_colors_and_restrictions() {
        let photo =
            types::UserProfilePhoto { has_video: true, personal: true, photo_id: 123, stripped_thumb: None, dc_id: 2 };
        let status = base::EmojiStatus::EmojiStatus(Box::new(types::EmojiStatus { document_id: 4, until: None }));
        let user = types::User {
            access_hash: Some(77),
            first_name: Some("Neko".into()),
            verified: true,
            support: true,
            restricted: true,
            restriction_reason: Some(vec![restriction()]),
            stories_hidden: true,
            usernames: Some(vec![username("old", false), username("neko", true)]),
            photo: Some(base::UserProfilePhoto::UserProfilePhoto(photo.into())),
            color: Some(color(5)),
            profile_color: Some(color(6)),
            emoji_status: Some(status.clone()),
            send_paid_messages_stars: Some(10),
            bot_verification_icon: Some(3),
            ..raw_user(7)
        };
        let entities = Entities { users: [(7, base::User::User(user.into()))].into(), chats: Default::default() };
        let chat = Chat::parse(&get_peer(7), &entities, false);

        assert_eq!(chat.photo, Some(ChatPhoto::new(7, 77, 123, 2, true, true)));
        assert_eq!((chat.dc_id, chat.username.as_deref(), chat.usernames.len()), (Some(2), Some("neko"), 2));
        assert_eq!(chat.reply_color, Some(ChatColor { color: Some(5), background_emoji_id: None }));
        assert_eq!(chat.profile_color.and_then(|c| c.color), Some(6));
        assert_eq!(chat.emoji_status, EmojiStatus::parse(&status));
        assert_eq!(chat.restrictions, [RestrictionReason::parse(&restriction())]);
        let flags = (chat.is_verified, chat.is_support, chat.is_restricted, chat.is_stories_hidden, chat.is_scam);
        assert_eq!(flags, (true, true, true, true, false));
        assert_eq!((chat.paid_message_star_count, chat.bot_verification_icon_custom_emoji_id), (Some(10), Some(3)));
        assert_eq!((chat.is_creator, chat.privileges, chat.is_banned), (false, None, false));
    }

    #[test]
    fn groups_carry_rights_calls_and_member_count() {
        // flags.4 is ban_users.
        let rights = types::ChatAdminRights::deserialize_bare(&mut &16i32.to_le_bytes()[..]).unwrap();
        let banned = base::ChatBannedRights::ChatBannedRights(ChatPermissions::default().write(0).into());
        let group = types::Chat {
            creator: true,
            left: false,
            deactivated: true,
            call_active: true,
            call_not_empty: false,
            noforwards: true,
            id: 3,
            title: "g".into(),
            photo: raw_photo(9, 4),
            participants_count: 12,
            date: 0,
            version: 1,
            migrated_to: None,
            admin_rights: Some(base::ChatAdminRights::ChatAdminRights(rights.into())),
            default_banned_rights: Some(banned.clone()),
        };
        let chat = parse_chat(-3, base::Chat::Chat(group.into()));

        assert_eq!(chat.photo, Some(ChatPhoto::new(-3, 0, 9, 4, false, false)));
        assert_eq!(chat.privileges, Some(ChatPrivileges { can_restrict_members: true, ..Default::default() }));
        assert_eq!(chat.permissions, Some(ChatPermissions::parse(&banned)));
        let flags = (chat.is_creator, chat.is_admin, chat.is_deactivated, chat.is_call_active, chat.is_call_not_empty);
        assert_eq!(flags, (true, true, true, true, false));
        assert_eq!((chat.members_count, chat.has_protected_content, chat.dc_id), (Some(12), true, Some(4)));
    }

    #[test]
    fn channels_carry_flags_level_and_subscription() {
        let channel = types::Channel {
            access_hash: Some(1),
            photo: raw_photo(8, 3),
            signatures: true,
            signature_profiles: true,
            scam: true,
            join_to_send: true,
            join_request: true,
            autotranslation: true,
            forum_tabs: true,
            broadcast_messages_allowed: true,
            stories_unavailable: true,
            participants_count: Some(40),
            level: Some(3),
            subscription_until_date: Some(99),
            linked_monoforum_id: Some(8),
            usernames: Some(vec![username("first", true), username("second", true)]),
            ..raw_channel(5)
        };
        let chat = parse_chat(get_channel_id(5), base::Chat::Channel(channel.into()));

        assert_eq!(chat.photo, Some(ChatPhoto::new(get_channel_id(5), 1, 8, 3, false, false)));
        assert_eq!(chat.username.as_deref(), Some("first"));
        assert_eq!((chat.sign_messages, chat.show_message_sender_name, chat.is_scam), (true, true, true));
        assert_eq!((chat.join_to_send_messages, chat.join_by_request), (true, true));
        assert_eq!((chat.has_automatic_translation, chat.has_forum_tabs), (true, true));
        assert_eq!((chat.has_direct_messages_group, chat.is_stories_unavailable), (true, true));
        assert_eq!((chat.level, chat.subscription_until_date, chat.members_count), (Some(3), Some(99), Some(40)));
        assert_eq!((chat.direct_messages_chat_id, chat.parent_chat), (Some(get_channel_id(8)), None));
    }

    #[test]
    fn forbidden_chats_are_banned_with_their_date() {
        let group = parse_chat(-3, base::Chat::ChatForbidden(types::ChatForbidden { id: 3, title: "g".into() }.into()));
        assert_eq!((group.is_banned, group.banned_until_date), (true, None));
        let forbidden = types::ChannelForbidden {
            broadcast: true,
            megagroup: false,
            monoforum: false,
            id: 5,
            access_hash: 1,
            title: "c".into(),
            until_date: Some(50),
        };
        let channel = parse_chat(get_channel_id(5), base::Chat::ChannelForbidden(forbidden.into()));
        assert_eq!((channel.is_banned, channel.banned_until_date), (true, Some(50)));
    }

    #[test]
    fn a_channel_without_access_hash_has_no_photo() {
        let channel = types::Channel { photo: raw_photo(8, 3), ..raw_channel(5) };
        let chat = parse_chat(get_channel_id(5), base::Chat::Channel(channel.into()));
        assert_eq!((chat.photo, chat.dc_id), (None, Some(3)));
    }

    #[test]
    fn a_forbidden_channel_keeps_its_kind() {
        let kind = |broadcast, megagroup| {
            let forbidden = types::ChannelForbidden {
                broadcast,
                megagroup,
                monoforum: false,
                id: 5,
                access_hash: 1,
                title: "t".into(),
                until_date: None,
            };
            let chats = [(get_channel_id(5), base::Chat::ChannelForbidden(Box::new(forbidden)))].into();
            let entities = Entities { users: Default::default(), chats };
            let chat = Chat::parse(&get_peer(get_channel_id(5)), &entities, false);
            (chat.kind, chat.title)
        };
        assert_eq!(kind(true, false), (ChatType::Channel, Some("t".into())));
        assert_eq!(kind(false, true), (ChatType::Supergroup, Some("t".into())));
    }

    #[test]
    fn admin_comes_from_admin_rights_on_groups_and_channels() {
        // flags.4 is ban_users.
        let rights = || {
            Some(base::ChatAdminRights::ChatAdminRights(
                types::ChatAdminRights::deserialize_bare(&mut &16i32.to_le_bytes()[..]).unwrap().into(),
            ))
        };
        let group = |admin_rights| types::Chat {
            creator: false,
            left: false,
            deactivated: false,
            call_active: false,
            call_not_empty: false,
            noforwards: false,
            id: 3,
            title: "g".into(),
            photo: base::ChatPhoto::ChatPhotoEmpty,
            participants_count: 2,
            date: 0,
            version: 1,
            migrated_to: None,
            admin_rights,
            default_banned_rights: None,
        };
        let channel = |admin_rights| {
            let mut buf = Vec::new();
            0u32.serialize(&mut buf);
            0u32.serialize(&mut buf);
            5i64.serialize(&mut buf);
            "c".to_owned().serialize(&mut buf);
            base::ChatPhoto::ChatPhotoEmpty.serialize(&mut buf);
            0i32.serialize(&mut buf);
            types::Channel { admin_rights, ..types::Channel::deserialize_bare(&mut &buf[..]).unwrap() }
        };
        let parse = |peer: i64, chat: base::Chat| {
            let entities = Entities { users: Default::default(), chats: [(peer, chat)].into() };
            Chat::parse(&get_peer(peer), &entities, false).is_admin
        };
        assert!(parse(-3, base::Chat::Chat(Box::new(group(rights())))));
        assert!(!parse(-3, base::Chat::Chat(Box::new(group(None)))));
        assert!(parse(get_channel_id(5), base::Chat::Channel(Box::new(channel(rights())))));
        assert!(!parse(get_channel_id(5), base::Chat::Channel(Box::new(channel(None)))));
    }
}
