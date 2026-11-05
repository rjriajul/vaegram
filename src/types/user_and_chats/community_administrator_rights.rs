use crate::raw::base;

/// What the account may do as an administrator of a community.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CommunityAdministratorRights {
    /// Use the community's management tools.
    pub can_manage_community: bool,
    /// Change the community's name, photo and other settings.
    pub can_change_info: bool,
    /// Add chats to the community and remove them.
    pub can_edit_chat_list: bool,
    /// Add other administrators.
    pub can_promote_members: bool,
    /// Ban members.
    pub can_ban_members: bool,
}

impl CommunityAdministratorRights {
    pub(crate) fn parse(raw: &base::ChatAdminRights) -> CommunityAdministratorRights {
        let base::ChatAdminRights::ChatAdminRights(r) = raw;
        CommunityAdministratorRights {
            can_manage_community: r.other,
            can_change_info: r.change_info,
            can_edit_chat_list: r.manage_linked_peers,
            can_promote_members: r.add_admins,
            can_ban_members: r.ban_users,
        }
    }
}
