use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::User;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatAdminWithInviteLinks {
    pub admin: User,
    pub chat_invite_links_count: i32,
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
