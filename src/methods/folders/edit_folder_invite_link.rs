use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{FolderInviteLink, PeerRef};

use super::check_folder_invite_link::slug;
use super::create_folder_invite_link::chatlist;

#[must_use = "a request does nothing until it is awaited"]
/// Request built by [`Client::edit_folder_invite_link`].
pub struct EditFolderInviteLink<'a> {
    client: &'a Client,
    folder_id: i32,
    slug: Result<String>,
    name: Option<String>,
    chat_ids: Option<Vec<PeerRef>>,
}

impl Client {
    /// Edits a folder invite link, given as the full link or its slug.
    ///
    /// Only the options set on the builder change. Awaiting returns the updated [`FolderInviteLink`].
    /// A link with no slug fails with `Error::InvalidArgument`.
    pub fn edit_folder_invite_link(&self, folder_id: i32, invite_link: &str) -> EditFolderInviteLink<'_> {
        let slug = slug(invite_link).map(str::to_owned);
        EditFolderInviteLink { client: self, folder_id, slug, name: None, chat_ids: None }
    }
}

impl EditFolderInviteLink<'_> {
    /// Renames the link.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Replaces the chats the link shares.
    pub fn chat_ids(mut self, chat_ids: impl IntoIterator<Item = impl Into<PeerRef>>) -> Self {
        self.chat_ids = Some(chat_ids.into_iter().map(Into::into).collect());
        self
    }

    async fn send(self) -> Result<FolderInviteLink> {
        let slug = self.slug?;
        let peers = match self.chat_ids {
            Some(chat_ids) => Some(self.client.resolve_peers(chat_ids).await?),
            None => None,
        };
        let (chatlist, title) = (chatlist(self.folder_id), self.name);
        let request = functions::chatlists::EditExportedInvite { chatlist, slug, title, peers };
        Ok(FolderInviteLink::parse(self.client.invoke(&request).await?))
    }
}

into_future!(EditFolderInviteLink, FolderInviteLink);
