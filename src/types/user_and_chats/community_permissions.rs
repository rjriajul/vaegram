use crate::raw::base;

/// What every member of a community can do.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct CommunityPermissions {
    /// Whether members can add chats to the community and remove them.
    pub can_edit_chat_list: bool,
}

impl CommunityPermissions {
    pub(crate) fn parse(raw: &base::ChatBannedRights) -> CommunityPermissions {
        let base::ChatBannedRights::ChatBannedRights(r) = raw;
        CommunityPermissions { can_edit_chat_list: !r.manage_linked_peers }
    }
}
