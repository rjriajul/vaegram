use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::FolderInviteLink;

use super::create_folder_invite_link::chatlist;

impl Client {
    pub async fn get_folder_invite_links(&self, folder_id: i32) -> Result<Vec<FolderInviteLink>> {
        let request = functions::chatlists::GetExportedInvites { chatlist: chatlist(folder_id) };
        let base::chatlists::ExportedInvites::ExportedInvites(r) = self.invoke(&request).await?;
        Entities::new(self, r.users, r.chats);
        Ok(r.invites.into_iter().map(FolderInviteLink::parse).collect())
    }
}
