use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::User;

/// An administrator and how many invite links they created in a chat.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ChatAdminWithInviteLinks {
    /// The administrator who created the links.
    pub admin: User,
    /// Number of invite links the administrator created.
    pub chat_invite_links_count: i32,
    /// Number of those links that were revoked.
    pub revoked_chat_invite_links_count: i32,
}

impl ChatAdminWithInviteLinks {
    pub(crate) fn parse(raw: types::ChatAdminWithInvites, entities: &Entities) -> ChatAdminWithInviteLinks {
        ChatAdminWithInviteLinks {
            admin: User::parse(raw.admin_id, entities),
            chat_invite_links_count: raw.invites_count,
            revoked_chat_invite_links_count: raw.revoked_invites_count,
        }
    }
}
