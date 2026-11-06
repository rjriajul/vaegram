use crate::dispatcher::Entities;
use crate::enums::ChatMemberStatus;
use crate::raw::base;
use crate::types::{Chat, ChatPermissions, ChatPrivileges, User};
use crate::utils::get_peer_id;

/// A member of a chat with their status and rights.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMember {
    /// The member as a user. For a banned or departed chat only the id is filled, see `chat`.
    pub user: User,
    /// The member's role or state in the chat.
    pub status: ChatMemberStatus,
    /// Whether the user is in the chat. `false` for banned members and members who left, restricted or not.
    pub is_member: bool,
    /// The member's custom title or rank. `None` if they have none.
    pub custom_title: Option<String>,
    /// Unix time the member joined. `None` for owners and members who left.
    pub joined_date: Option<i32>,
    /// Unix time a restriction or ban ends, or a paid subscription expires. `None` if there is none.
    pub until_date: Option<i32>,
    /// Administrator rights. Set only for administrators and owners of supergroups and channels.
    pub privileges: Option<ChatPrivileges>,
    /// Permissions of a restricted member. `None` for anyone else.
    pub permissions: Option<ChatPermissions>,
    /// The banned or departed member when it is a chat rather than a user. `user` then holds only its id.
    pub chat: Option<Chat>,
    /// Who invited the member. `None` for owners, banned members, members who left, and when Telegram omits it.
    pub invited_by: Option<User>,
    /// Who promoted the administrator. Set only for administrators of supergroups and channels.
    pub promoted_by: Option<User>,
    /// Who restricted or banned the member. `None` for other members.
    pub restricted_by: Option<User>,
    /// Whether you can change this administrator's rights. Always `false` for other members.
    pub can_be_edited: bool,
}

impl ChatMember {
    pub(crate) fn new(user_id: i64, status: ChatMemberStatus, entities: &Entities) -> ChatMember {
        ChatMember {
            user: User::parse(user_id, entities),
            status,
            is_member: !matches!(status, ChatMemberStatus::Left | ChatMemberStatus::Banned),
            custom_title: None,
            joined_date: None,
            until_date: None,
            privileges: None,
            permissions: None,
            chat: None,
            invited_by: None,
            promoted_by: None,
            restricted_by: None,
            can_be_edited: false,
        }
    }

    // Banned and departed members can be channels or groups, not just users.
    fn non_user_chat(peer: &base::Peer, entities: &Entities) -> Option<Chat> {
        match peer {
            base::Peer::PeerUser(_) => None,
            _ => Some(Chat::parse(peer, entities, false)),
        }
    }

    pub(crate) fn parse_channel(participant: &base::ChannelParticipant, entities: &Entities) -> ChatMember {
        use base::ChannelParticipant as P;
        match participant {
            P::ChannelParticipant(p) => ChatMember {
                custom_title: p.rank.clone(),
                joined_date: Some(p.date),
                until_date: p.subscription_until_date,
                ..Self::new(p.user_id, ChatMemberStatus::Member, entities)
            },
            P::ChannelParticipantSelf(p) => ChatMember {
                custom_title: p.rank.clone(),
                joined_date: Some(p.date),
                until_date: p.subscription_until_date,
                invited_by: Some(User::parse(p.inviter_id, entities)),
                ..Self::new(p.user_id, ChatMemberStatus::Member, entities)
            },
            P::ChannelParticipantCreator(p) => ChatMember {
                custom_title: p.rank.clone(),
                privileges: Some(ChatPrivileges::parse(&p.admin_rights)),
                ..Self::new(p.user_id, ChatMemberStatus::Owner, entities)
            },
            P::ChannelParticipantAdmin(p) => ChatMember {
                custom_title: p.rank.clone(),
                joined_date: Some(p.date),
                privileges: Some(ChatPrivileges::parse(&p.admin_rights)),
                invited_by: p.inviter_id.map(|id| User::parse(id, entities)),
                promoted_by: Some(User::parse(p.promoted_by, entities)),
                can_be_edited: p.can_edit,
                ..Self::new(p.user_id, ChatMemberStatus::Administrator, entities)
            },
            P::ChannelParticipantBanned(p) => {
                let base::ChatBannedRights::ChatBannedRights(rights) = &p.banned_rights;
                let (status, permissions) = if rights.view_messages {
                    (ChatMemberStatus::Banned, None)
                } else {
                    (ChatMemberStatus::Restricted, Some(ChatPermissions::parse(&p.banned_rights)))
                };
                ChatMember {
                    custom_title: p.rank.clone(),
                    joined_date: Some(p.date),
                    until_date: (rights.until_date != 0).then_some(rights.until_date),
                    permissions,
                    is_member: !p.left && !rights.view_messages,
                    chat: Self::non_user_chat(&p.peer, entities),
                    restricted_by: Some(User::parse(p.kicked_by, entities)),
                    ..Self::new(get_peer_id(&p.peer), status, entities)
                }
            }
            P::ChannelParticipantLeft(p) => ChatMember {
                chat: Self::non_user_chat(&p.peer, entities),
                ..Self::new(get_peer_id(&p.peer), ChatMemberStatus::Left, entities)
            },
        }
    }

    pub(crate) fn parse_chat(participant: &base::ChatParticipant, entities: &Entities) -> ChatMember {
        use base::ChatParticipant as P;
        match participant {
            P::ChatParticipant(p) => ChatMember {
                custom_title: p.rank.clone(),
                joined_date: Some(p.date),
                invited_by: Some(User::parse(p.inviter_id, entities)),
                ..Self::new(p.user_id, ChatMemberStatus::Member, entities)
            },
            P::ChatParticipantCreator(p) => {
                ChatMember { custom_title: p.rank.clone(), ..Self::new(p.user_id, ChatMemberStatus::Owner, entities) }
            }
            P::ChatParticipantAdmin(p) => ChatMember {
                custom_title: p.rank.clone(),
                joined_date: Some(p.date),
                invited_by: Some(User::parse(p.inviter_id, entities)),
                ..Self::new(p.user_id, ChatMemberStatus::Administrator, entities)
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ChatMember;
    use crate::dispatcher::Entities;
    use crate::enums::ChatMemberStatus;
    use crate::raw::{base, types};
    use crate::types::{Chat, ChatPermissions, ChatPrivileges};
    use crate::utils::{get_channel_id, get_peer};

    fn banned(view_messages: bool, until_date: i32) -> base::ChannelParticipant {
        let mut rights = ChatPermissions::new().can_send_messages(true).write(until_date);
        rights.view_messages = view_messages;
        base::ChannelParticipant::ChannelParticipantBanned(Box::new(types::ChannelParticipantBanned {
            left: false,
            peer: base::Peer::PeerUser(types::PeerUser { user_id: 7 }),
            kicked_by: 1,
            date: 100,
            banned_rights: base::ChatBannedRights::ChatBannedRights(Box::new(rights)),
            rank: None,
        }))
    }

    #[test]
    fn banned_participant_is_banned_or_restricted() {
        let entities = Entities::default();
        let member = ChatMember::parse_channel(&banned(true, 0), &entities);
        assert_eq!((member.user.id, member.status, member.until_date), (7, ChatMemberStatus::Banned, None));
        assert_eq!((member.permissions, member.is_member), (None, false));

        let member = ChatMember::parse_channel(&banned(false, 200), &entities);
        assert_eq!((member.status, member.until_date), (ChatMemberStatus::Restricted, Some(200)));
        assert!(member.is_member);
        assert_eq!(member.permissions.map(|p| (p.can_send_messages, p.can_send_photos)), Some((true, false)));
    }

    #[test]
    fn admins_carry_promoter_inviter_and_editability() {
        let entities = Entities::default();
        let admin = |can_edit, inviter_id| {
            base::ChannelParticipant::ChannelParticipantAdmin(Box::new(types::ChannelParticipantAdmin {
                can_edit,
                is_self: false,
                user_id: 7,
                inviter_id,
                promoted_by: 3,
                date: 100,
                admin_rights: ChatPrivileges::default().write(),
                rank: None,
            }))
        };
        let member = ChatMember::parse_channel(&admin(true, Some(4)), &entities);
        let ids = (member.invited_by.map(|u| u.id), member.promoted_by.map(|u| u.id), member.can_be_edited);
        assert_eq!(ids, (Some(4), Some(3), true));
        assert_eq!((member.restricted_by, member.chat), (None, None));

        let member = ChatMember::parse_channel(&admin(false, None), &entities);
        assert_eq!((member.invited_by, member.can_be_edited), (None, false));
    }

    #[test]
    fn restricted_members_name_who_restricted_them() {
        let member = ChatMember::parse_channel(&banned(false, 0), &Entities::default());
        assert_eq!((member.status, member.restricted_by.map(|u| u.id)), (ChatMemberStatus::Restricted, Some(1)));
        assert_eq!((member.chat, member.promoted_by, member.invited_by), (None, None, None));
    }

    #[test]
    fn banned_chats_fill_chat_not_just_an_id() {
        let entities = Entities::default();
        let channel = get_peer(get_channel_id(9));
        let base::ChannelParticipant::ChannelParticipantBanned(mut raw) = banned(true, 0) else { unreachable!() };
        raw.peer = channel.clone();
        let member = ChatMember::parse_channel(&base::ChannelParticipant::ChannelParticipantBanned(raw), &entities);
        assert_eq!((member.user.id, member.status), (get_channel_id(9), ChatMemberStatus::Banned));
        assert_eq!(member.chat, Some(Chat::parse(&channel, &entities, false)));

        let left = types::ChannelParticipantLeft { peer: channel.clone() };
        let member =
            ChatMember::parse_channel(&base::ChannelParticipant::ChannelParticipantLeft(Box::new(left)), &entities);
        assert_eq!(member.chat, Some(Chat::parse(&channel, &entities, false)));

        let left = types::ChannelParticipantLeft { peer: get_peer(7) };
        let member =
            ChatMember::parse_channel(&base::ChannelParticipant::ChannelParticipantLeft(Box::new(left)), &entities);
        assert_eq!((member.user.id, member.chat), (7, None));
    }

    #[test]
    fn group_members_carry_their_inviter() {
        let entities = Entities::default();
        let member = types::ChatParticipant { user_id: 7, inviter_id: 4, date: 100, rank: None };
        let member = ChatMember::parse_chat(&base::ChatParticipant::ChatParticipant(Box::new(member)), &entities);
        assert_eq!((member.status, member.invited_by.map(|u| u.id)), (ChatMemberStatus::Member, Some(4)));
        let admin = types::ChatParticipantAdmin { user_id: 8, inviter_id: 5, date: 100, rank: None };
        let admin = ChatMember::parse_chat(&base::ChatParticipant::ChatParticipantAdmin(Box::new(admin)), &entities);
        assert_eq!(admin.invited_by.map(|u| u.id), Some(5));
        assert_eq!((admin.promoted_by, admin.can_be_edited), (None, false));
    }
}
