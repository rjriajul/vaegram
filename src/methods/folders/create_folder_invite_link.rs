use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::{FolderInviteLink, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct CreateFolderInviteLink<'a> {
    client: &'a Client,
    folder_id: i32,
    chat_ids: Vec<PeerRef>,
    name: String,
}

impl Client {
    pub fn create_folder_invite_link(
        &self,
        folder_id: i32,
        chat_ids: impl IntoIterator<Item = impl Into<PeerRef>>,
    ) -> CreateFolderInviteLink<'_> {
        let chat_ids = chat_ids.into_iter().map(Into::into).collect();
        CreateFolderInviteLink { client: self, folder_id, chat_ids, name: String::new() }
    }
}

impl CreateFolderInviteLink<'_> {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    async fn send(self) -> Result<FolderInviteLink> {
        if self.chat_ids.is_empty() {
            return Err(Error::InvalidArgument("create_folder_invite_link needs at least one chat"));
        }
        let peers = self.client.resolve_peers(self.chat_ids).await?;
        let chatlist = chatlist(self.folder_id);
        let request = functions::chatlists::ExportChatlistInvite { chatlist, title: self.name, peers };
        let base::chatlists::ExportedChatlistInvite::ExportedChatlistInvite(r) = self.client.invoke(&request).await?;
        Ok(FolderInviteLink::parse(r.invite))
    }
}

into_future!(CreateFolderInviteLink, FolderInviteLink);

pub(super) fn chatlist(folder_id: i32) -> base::InputChatlist {
    base::InputChatlist::InputChatlistDialogFilter(types::InputChatlistDialogFilter { filter_id: folder_id })
}
