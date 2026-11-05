use crate::raw::base;
use crate::utils::get_peer_id;

/// An invite link that shares a folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderInviteLink {
    /// The link.
    pub invite_link: String,
    /// The name given to the link.
    pub name: String,
    /// Ids of the chats the link adds.
    pub chat_ids: Vec<i64>,
}

impl FolderInviteLink {
    pub(crate) fn parse(invite: base::ExportedChatlistInvite) -> FolderInviteLink {
        let base::ExportedChatlistInvite::ExportedChatlistInvite(i) = invite;
        FolderInviteLink { invite_link: i.url, name: i.title, chat_ids: i.peers.iter().map(get_peer_id).collect() }
    }
}
