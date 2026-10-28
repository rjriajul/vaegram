use crate::dispatcher::Entities;
use crate::enums::ChatMemberStatus;
use crate::raw::base;
use crate::types::{ChatPermissions, ChatPrivileges, User};
use crate::utils::get_peer_id;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMember {
    pub user: User,
    pub status: ChatMemberStatus,
    pub is_member: bool,
    pub custom_title: Option<String>,
    pub joined_date: Option<i32>,
    pub until_date: Option<i32>,
    pub privileges: Option<ChatPrivileges>,
    pub permissions: Option<ChatPermissions>,
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
                    ..Self::new(get_peer_id(&p.peer), status, entities)
                }
            }
            P::ChannelParticipantLeft(p) => Self::new(get_peer_id(&p.peer), ChatMemberStatus::Left, entities),
        }
    }

    pub(crate) fn parse_chat(participant: &base::ChatParticipant, entities: &Entities) -> ChatMember {
        use base::ChatParticipant as P;
        match participant {
            P::ChatParticipant(p) => ChatMember {
                custom_title: p.rank.clone(),
                joined_date: Some(p.date),
                ..Self::new(p.user_id, ChatMemberStatus::Member, entities)
            },
            P::ChatParticipantCreator(p) => {
                ChatMember { custom_title: p.rank.clone(), ..Self::new(p.user_id, ChatMemberStatus::Owner, entities) }
            }
            P::ChatParticipantAdmin(p) => ChatMember {
                custom_title: p.rank.clone(),
                joined_date: Some(p.date),
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
    use crate::types::ChatPermissions;

    fn banned(view_messages: bool, until_date: i32) -> base::ChannelParticipant {
        let mut rights = ChatPermissions { can_send_messages: true, ..Default::default() }.write(until_date);
        rights.view_messages = view_messages;
        base::ChannelParticipant::ChannelParticipantBanned(Box::new(types::ChannelParticipantBanned {
            left: false,
            peer: base::Peer::PeerUser(Box::new(types::PeerUser { user_id: 7 })),
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
}
