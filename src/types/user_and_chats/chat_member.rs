use crate::dispatcher::Entities;
use crate::enums::ChatMemberStatus;
use crate::raw::base;
use crate::types::{Chat, ChatPermissions, ChatPrivileges, User};
use crate::utils::get_peer_id;

/// A member of a chat with their status and rights.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ChatMember {
    /// The member as a user. For a banned or departed chat only the id is filled, see `chat`.
    pub user: User,
    /// The member's role in the chat, with what only that role has. [`status`](ChatMember::status) gives just its
    /// kind.
    pub role: ChatMemberRole,
    /// Whether the user is in the chat. `false` for banned members and members who left, restricted or not.
    pub is_member: bool,
    /// The member's custom title or rank. `None` if they have none.
    pub custom_title: Option<String>,
    /// Unix time the member joined. `None` for owners and members who left.
    pub joined_date: Option<i32>,
    /// The banned or departed member when it is a chat rather than a user. `user` then holds only its id.
    pub chat: Option<Chat>,
    /// Who invited the member. `None` for owners, banned members, members who left, and when Telegram omits it.
    pub invited_by: Option<User>,
}

/// A member's role in a chat, with the details that come with it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum ChatMemberRole {
    /// The chat's owner.
    Owner {
        /// The owner's rights, in supergroups and channels. `None` in basic groups.
        privileges: Option<ChatPrivileges>,
    },
    /// An administrator other than the owner.
    Administrator {
        /// The administrator's rights, in supergroups and channels. `None` in basic groups.
        privileges: Option<ChatPrivileges>,
        /// Who promoted them, in supergroups and channels.
        promoted_by: Option<User>,
        /// Whether you can change their rights.
        can_be_edited: bool,
    },
    /// A regular member.
    Member {
        /// Unix time a paid subscription expires. `None` without one.
        until_date: Option<i32>,
    },
    /// Has some permissions taken away but can still view the chat. A restricted user can be outside the chat,
    /// see [`ChatMember::is_member`].
    Restricted {
        /// What the member may still do.
        permissions: ChatPermissions,
        /// Unix time the restriction ends. `None` if it is forever.
        until_date: Option<i32>,
        /// Who restricted them.
        restricted_by: Option<User>,
    },
    /// Not in the chat.
    Left,
    /// Barred from viewing the chat.
    Banned {
        /// Unix time the ban ends. `None` if it is forever.
        until_date: Option<i32>,
        /// Who banned them.
        banned_by: Option<User>,
    },
}

impl ChatMemberRole {
    /// The role's kind, without its details.
    pub fn status(&self) -> ChatMemberStatus {
        match self {
            ChatMemberRole::Owner { .. } => ChatMemberStatus::Owner,
            ChatMemberRole::Administrator { .. } => ChatMemberStatus::Administrator,
            ChatMemberRole::Member { .. } => ChatMemberStatus::Member,
            ChatMemberRole::Restricted { .. } => ChatMemberStatus::Restricted,
            ChatMemberRole::Left => ChatMemberStatus::Left,
            ChatMemberRole::Banned { .. } => ChatMemberStatus::Banned,
        }
    }

    /// The bare role of a status, with no details.
    pub(crate) fn of(status: ChatMemberStatus) -> ChatMemberRole {
        match status {
            ChatMemberStatus::Owner => ChatMemberRole::Owner { privileges: None },
            ChatMemberStatus::Administrator => {
                ChatMemberRole::Administrator { privileges: None, promoted_by: None, can_be_edited: false }
            }
            ChatMemberStatus::Restricted => ChatMemberRole::Restricted {
                permissions: ChatPermissions::default(),
                until_date: None,
                restricted_by: None,
            },
            ChatMemberStatus::Left => ChatMemberRole::Left,
            ChatMemberStatus::Banned => ChatMemberRole::Banned { until_date: None, banned_by: None },
            ChatMemberStatus::Member => ChatMemberRole::Member { until_date: None },
        }
    }
}

impl ChatMember {
    pub(crate) fn new(user_id: i64, role: ChatMemberRole, entities: &Entities) -> ChatMember {
        ChatMember {
            user: User::parse(user_id, entities),
            is_member: !matches!(role, ChatMemberRole::Left | ChatMemberRole::Banned { .. }),
            role,
            custom_title: None,
            joined_date: None,
            chat: None,
            invited_by: None,
        }
    }

    /// The kind of the member's [`role`](ChatMember::role).
    pub fn status(&self) -> ChatMemberStatus {
        self.role.status()
    }

    /// Administrator rights of an owner or administrator of a supergroup or channel. `None` for anyone else.
    pub fn privileges(&self) -> Option<&ChatPrivileges> {
        match &self.role {
            ChatMemberRole::Owner { privileges } | ChatMemberRole::Administrator { privileges, .. } => {
                privileges.as_ref()
            }
            _ => None,
        }
    }

    /// Permissions of a restricted member. `None` for anyone else.
    pub fn permissions(&self) -> Option<&ChatPermissions> {
        match &self.role {
            ChatMemberRole::Restricted { permissions, .. } => Some(permissions),
            _ => None,
        }
    }

    /// Unix time a restriction or ban ends, or a paid subscription expires. `None` if there is none.
    pub fn until_date(&self) -> Option<i32> {
        match self.role {
            ChatMemberRole::Member { until_date }
            | ChatMemberRole::Restricted { until_date, .. }
            | ChatMemberRole::Banned { until_date, .. } => until_date,
            _ => None,
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
                ..Self::new(p.user_id, ChatMemberRole::Member { until_date: p.subscription_until_date }, entities)
            },
            P::ChannelParticipantSelf(p) => ChatMember {
                custom_title: p.rank.clone(),
                joined_date: Some(p.date),
                invited_by: Some(User::parse(p.inviter_id, entities)),
                ..Self::new(p.user_id, ChatMemberRole::Member { until_date: p.subscription_until_date }, entities)
            },
            P::ChannelParticipantCreator(p) => {
                let role = ChatMemberRole::Owner { privileges: Some(ChatPrivileges::parse(&p.admin_rights)) };
                ChatMember { custom_title: p.rank.clone(), ..Self::new(p.user_id, role, entities) }
            }
            P::ChannelParticipantAdmin(p) => {
                let role = ChatMemberRole::Administrator {
                    privileges: Some(ChatPrivileges::parse(&p.admin_rights)),
                    promoted_by: Some(User::parse(p.promoted_by, entities)),
                    can_be_edited: p.can_edit,
                };
                ChatMember {
                    custom_title: p.rank.clone(),
                    joined_date: Some(p.date),
                    invited_by: p.inviter_id.map(|id| User::parse(id, entities)),
                    ..Self::new(p.user_id, role, entities)
                }
            }
            P::ChannelParticipantBanned(p) => {
                let base::ChatBannedRights::ChatBannedRights(rights) = &p.banned_rights;
                let until_date = (rights.until_date != 0).then_some(rights.until_date);
                let by = Some(User::parse(p.kicked_by, entities));
                let role = if rights.view_messages {
                    ChatMemberRole::Banned { until_date, banned_by: by }
                } else {
                    let permissions = ChatPermissions::parse(&p.banned_rights);
                    ChatMemberRole::Restricted { permissions, until_date, restricted_by: by }
                };
                ChatMember {
                    custom_title: p.rank.clone(),
                    joined_date: Some(p.date),
                    is_member: !p.left && !rights.view_messages,
                    chat: Self::non_user_chat(&p.peer, entities),
                    ..Self::new(get_peer_id(&p.peer), role, entities)
                }
            }
            P::ChannelParticipantLeft(p) => ChatMember {
                chat: Self::non_user_chat(&p.peer, entities),
                ..Self::new(get_peer_id(&p.peer), ChatMemberRole::Left, entities)
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
                ..Self::new(p.user_id, ChatMemberRole::Member { until_date: None }, entities)
            },
            P::ChatParticipantCreator(p) => ChatMember {
                custom_title: p.rank.clone(),
                ..Self::new(p.user_id, ChatMemberRole::of(ChatMemberStatus::Owner), entities)
            },
            P::ChatParticipantAdmin(p) => ChatMember {
                custom_title: p.rank.clone(),
                joined_date: Some(p.date),
                invited_by: Some(User::parse(p.inviter_id, entities)),
                ..Self::new(p.user_id, ChatMemberRole::of(ChatMemberStatus::Administrator), entities)
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ChatMember, ChatMemberRole};
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
        assert_eq!((member.user.id, member.status(), member.until_date()), (7, ChatMemberStatus::Banned, None));
        assert_eq!((member.permissions(), member.is_member), (None, false));

        let member = ChatMember::parse_channel(&banned(false, 200), &entities);
        assert_eq!((member.status(), member.until_date()), (ChatMemberStatus::Restricted, Some(200)));
        assert!(member.is_member);
        assert_eq!(member.permissions().map(|p| (p.can_send_messages, p.can_send_photos)), Some((true, false)));
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
        assert_eq!((member.invited_by.map(|u| u.id), member.chat), (Some(4), None));
        let ChatMemberRole::Administrator { promoted_by, can_be_edited, .. } = member.role else { panic!() };
        assert_eq!((promoted_by.map(|u| u.id), can_be_edited), (Some(3), true));

        let member = ChatMember::parse_channel(&admin(false, None), &entities);
        let can_be_edited = matches!(member.role, ChatMemberRole::Administrator { can_be_edited: true, .. });
        assert_eq!((member.invited_by, can_be_edited), (None, false));
    }

    #[test]
    fn restricted_members_name_who_restricted_them() {
        let member = ChatMember::parse_channel(&banned(false, 0), &Entities::default());
        let ChatMemberRole::Restricted { restricted_by, .. } = &member.role else { panic!("{:?}", member.role) };
        assert_eq!(restricted_by.as_ref().map(|u| u.id), Some(1));
        assert_eq!((member.chat, member.invited_by), (None, None));
    }

    #[test]
    fn banned_chats_fill_chat_not_just_an_id() {
        let entities = Entities::default();
        let channel = get_peer(get_channel_id(9));
        let base::ChannelParticipant::ChannelParticipantBanned(mut raw) = banned(true, 0) else { unreachable!() };
        raw.peer = channel.clone();
        let member = ChatMember::parse_channel(&base::ChannelParticipant::ChannelParticipantBanned(raw), &entities);
        assert_eq!((member.user.id, member.status()), (get_channel_id(9), ChatMemberStatus::Banned));
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
        assert_eq!((member.status(), member.invited_by.map(|u| u.id)), (ChatMemberStatus::Member, Some(4)));
        let admin = types::ChatParticipantAdmin { user_id: 8, inviter_id: 5, date: 100, rank: None };
        let admin = ChatMember::parse_chat(&base::ChatParticipant::ChatParticipantAdmin(Box::new(admin)), &entities);
        assert_eq!(admin.invited_by.map(|u| u.id), Some(5));
        let role = ChatMemberRole::Administrator { privileges: None, promoted_by: None, can_be_edited: false };
        assert_eq!(admin.role, role);
    }
}
