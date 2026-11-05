use crate::raw::base;

/// What every member of a community may do.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CommunityPermissions {
    /// Members can add chats to the community and remove them.
    pub can_edit_chat_list: bool,
}

impl CommunityPermissions {
    pub(crate) fn parse(raw: &base::ChatBannedRights) -> CommunityPermissions {
        let base::ChatBannedRights::ChatBannedRights(r) = raw;
        CommunityPermissions { can_edit_chat_list: !r.manage_linked_peers }
    }
}
