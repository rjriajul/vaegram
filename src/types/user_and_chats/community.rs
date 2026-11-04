use crate::dispatcher::Entities;
use crate::enums::CommunityMemberStatus;
use crate::raw::base;
use crate::types::{ChatPhoto, CommunityAdministratorRights, CommunityPermissions};
use crate::utils::get_channel_id;

/// A community; every field after `name` is empty for a community the account has never seen.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Community {
    pub id: i64,
    pub name: String,
    /// The account can open the community; `false` when Telegram sent only its name.
    pub have_access: bool,
    pub photo: Option<ChatPhoto>,
    pub date: Option<i32>,
    pub status: Option<CommunityMemberStatus>,
    /// Your rights as an administrator.
    pub administrator_rights: Option<CommunityAdministratorRights>,
    /// The default rights of members.
    pub permissions: Option<CommunityPermissions>,
}

impl Community {
    // Communities live in the channel id space, so they share the -100 Bot API prefix.
    pub(crate) fn parse(community_id: i64, entities: &Entities) -> Community {
        let id = get_channel_id(community_id);
        match entities.chats.get(&id) {
            Some(base::Chat::Community(c)) => {
                let photo = match (&c.photo, c.access_hash) {
                    (base::ChatPhoto::ChatPhoto(p), Some(hash)) => {
                        Some(ChatPhoto::new(id, hash, p.photo_id, p.dc_id, p.has_video, false))
                    }
                    _ => None,
                };
                // A min community carries no flags of the account's own, so its status and rights are unknown.
                let status = if c.min {
                    None
                } else if c.creator {
                    Some(CommunityMemberStatus::Creator)
                } else if c.admin_rights.is_some() {
                    Some(CommunityMemberStatus::Administrator)
                } else if c.left {
                    Some(CommunityMemberStatus::Left)
                } else {
                    Some(CommunityMemberStatus::Member)
                };
                let administrator_rights = c.admin_rights.as_ref().filter(|_| !c.min);
                Community {
                    id,
                    name: c.title.clone(),
                    have_access: c.access_hash.is_some(),
                    photo,
                    date: Some(c.date),
                    status,
                    administrator_rights: administrator_rights.map(CommunityAdministratorRights::parse),
                    permissions: c.default_banned_rights.as_ref().map(CommunityPermissions::parse),
                }
            }
            Some(base::Chat::CommunityForbidden(c)) => Community {
                id,
                name: c.title.clone(),
                status: Some(CommunityMemberStatus::Banned),
                ..Community::default()
            },
            _ => Community { id, ..Community::default() },
        }
    }

    pub(crate) fn linked(chat_id: i64, entities: &Entities) -> Option<Community> {
        let community_id = match (entities.users.get(&chat_id), entities.chats.get(&chat_id)) {
            (Some(base::User::User(u)), _) => u.linked_community_id,
            (_, Some(base::Chat::Channel(c))) => c.linked_community_id,
            _ => None,
        }?;
        Some(Community::parse(community_id, entities))
    }
}

#[cfg(test)]
mod tests {
    use super::Community;
    use crate::dispatcher::Entities;
    use crate::enums::CommunityMemberStatus as Status;
    use crate::raw::{base, types};
    use crate::utils::get_channel_id;

    fn raw(f: impl FnOnce(&mut types::Community)) -> base::Chat {
        let mut c = types::Community {
            creator: false,
            left: false,
            min: false,
            collapsed_in_dialogs: false,
            id: 7,
            access_hash: Some(99),
            title: "Hub".into(),
            photo: base::ChatPhoto::ChatPhotoEmpty,
            date: 1234,
            admin_rights: None,
            default_banned_rights: None,
        };
        f(&mut c);
        base::Chat::Community(Box::new(c))
    }

    fn parse(chat: base::Chat) -> Community {
        let client = crate::Client::offline();
        let entities = Entities::new(&client, Vec::new(), vec![chat]);
        Community::parse(7, &entities)
    }

    fn admin_rights(f: impl FnOnce(&mut types::ChatAdminRights)) -> base::ChatAdminRights {
        let mut r = types::ChatAdminRights {
            change_info: false,
            post_messages: false,
            edit_messages: false,
            delete_messages: false,
            ban_users: false,
            invite_users: false,
            pin_messages: false,
            add_admins: false,
            anonymous: false,
            manage_call: false,
            other: false,
            manage_topics: false,
            post_stories: false,
            edit_stories: false,
            delete_stories: false,
            manage_direct_messages: false,
            manage_ranks: false,
            manage_linked_peers: false,
            manage_welcome_messages: false,
        };
        f(&mut r);
        base::ChatAdminRights::ChatAdminRights(Box::new(r))
    }

    #[test]
    fn status_follows_creator_admin_left_member() {
        let status = |chat| parse(chat).status;
        assert_eq!(status(raw(|c| c.creator = true)), Some(Status::Creator));
        let rights = admin_rights(|_| {});
        assert_eq!(status(raw(|c| c.admin_rights = Some(rights.clone()))), Some(Status::Administrator));
        let left_admin = raw(|c| (c.left, c.admin_rights) = (true, Some(rights.clone())));
        assert_eq!(status(left_admin), Some(Status::Administrator));
        assert_eq!(status(raw(|c| c.left = true)), Some(Status::Left));
        assert_eq!(status(raw(|_| {})), Some(Status::Member));
        let parsed = parse(raw(|_| {}));
        assert_eq!(
            (parsed.id, parsed.name.as_str(), parsed.date, parsed.have_access),
            (get_channel_id(7), "Hub", Some(1234), true)
        );
    }

    #[test]
    fn min_community_has_no_status_or_rights() {
        let rights = admin_rights(|r| r.other = true);
        let parsed = parse(raw(|c| (c.min, c.creator, c.admin_rights) = (true, true, Some(rights))));
        assert_eq!((parsed.status, parsed.administrator_rights), (None, None));
        assert_eq!(parsed.name, "Hub");
    }

    #[test]
    fn forbidden_is_banned_without_access() {
        let forbidden = types::CommunityForbidden { id: 7, access_hash: None, title: "Hub".into() };
        let parsed = parse(base::Chat::CommunityForbidden(Box::new(forbidden)));
        assert_eq!((parsed.have_access, parsed.status, parsed.date), (false, Some(Status::Banned), None));
        assert_eq!(parsed.name, "Hub");
    }

    #[test]
    fn photo_needs_access_hash() {
        let photo = || {
            let p = types::ChatPhoto { has_video: false, photo_id: 5, stripped_thumb: None, dc_id: 2 };
            base::ChatPhoto::ChatPhoto(Box::new(p))
        };
        let with = parse(raw(|c| c.photo = photo()));
        assert!(with.photo.is_some());
        let without = parse(raw(|c| (c.photo, c.access_hash) = (photo(), None)));
        assert_eq!((without.photo, without.have_access), (None, false));
        assert_eq!(parse(raw(|_| {})).photo, None);
    }

    #[test]
    fn rights_and_permissions_map_flags() {
        let rights = admin_rights(|r| {
            r.other = true;
            r.manage_linked_peers = true;
            r.ban_users = true;
        });
        let banned = |manage_linked_peers| {
            let r = types::ChatBannedRights {
                view_messages: false,
                send_messages: false,
                send_media: false,
                send_stickers: false,
                send_gifs: false,
                send_games: false,
                send_inline: false,
                embed_links: false,
                send_polls: false,
                change_info: false,
                invite_users: false,
                pin_messages: false,
                manage_topics: false,
                send_photos: false,
                send_videos: false,
                send_roundvideos: false,
                send_audios: false,
                send_voices: false,
                send_docs: false,
                send_plain: false,
                edit_rank: false,
                send_reactions: false,
                manage_linked_peers,
                until_date: 0,
            };
            base::ChatBannedRights::ChatBannedRights(Box::new(r))
        };
        let parsed = parse(raw(|c| (c.admin_rights, c.default_banned_rights) = (Some(rights), Some(banned(true)))));
        let rights = parsed.administrator_rights.unwrap();
        assert_eq!(
            (rights.can_manage_community, rights.can_change_info, rights.can_edit_chat_list),
            (true, false, true)
        );
        assert_eq!((rights.can_promote_members, rights.can_ban_members), (false, true));
        assert_eq!(parsed.permissions.map(|p| p.can_edit_chat_list), Some(false));
        let open = parse(raw(|c| c.default_banned_rights = Some(banned(false))));
        assert_eq!(open.permissions.map(|p| p.can_edit_chat_list), Some(true));
    }

    #[test]
    fn unknown_community_keeps_only_id() {
        let parsed = Community::parse(7, &Entities::default());
        assert_eq!(parsed, Community { id: get_channel_id(7), ..Community::default() });
    }
}
