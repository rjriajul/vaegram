use crate::raw::base;
use crate::utils::get_peer_id;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderInviteLink {
    pub invite_link: String,
    pub name: String,
    pub chat_ids: Vec<i64>,
}

impl FolderInviteLink {
    pub(crate) fn parse(invite: base::ExportedChatlistInvite) -> FolderInviteLink {
        let base::ExportedChatlistInvite::ExportedChatlistInvite(i) = invite;
        FolderInviteLink { invite_link: i.url, name: i.title, chat_ids: i.peers.iter().map(get_peer_id).collect() }
    }
}
