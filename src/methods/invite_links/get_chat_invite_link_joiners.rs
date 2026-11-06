use super::get_chat_join_requests::Importers;
use crate::Client;
use crate::errors::Result;
use crate::types::{ChatJoiner, PeerRef};

/// Users who joined through an invite link, built by [`Client::get_chat_invite_link_joiners`].
///
/// Awaiting it collects the joiners into a `Vec`, up to [`limit`](GetChatInviteLinkJoiners::limit);
/// [`next`](GetChatInviteLinkJoiners::next) reads them one at a time. Joiners are fetched 100 to a page.
#[must_use = "a pager does nothing until it is awaited or read with next()"]
pub struct GetChatInviteLinkJoiners<'a> {
    client: &'a Client,
    importers: Importers,
}

impl Client {
    /// Lists the users who joined a chat through an invite link. User accounts only.
    pub fn get_chat_invite_link_joiners(
        &self,
        chat_id: impl Into<PeerRef>,
        invite_link: impl Into<String>,
    ) -> GetChatInviteLinkJoiners<'_> {
        let importers = Importers::new(chat_id.into(), false, Some(invite_link.into()));
        GetChatInviteLinkJoiners { client: self, importers }
    }
}

impl GetChatInviteLinkJoiners<'_> {
    /// Stops after this many joiners in total. The default reads them all.
    pub fn limit(mut self, limit: usize) -> Self {
        self.importers.pages.remaining = limit;
        self
    }

    /// Returns the next joiner, or `None` once the joiners or the limit run out.
    pub async fn next(&mut self) -> Result<Option<ChatJoiner>> {
        self.importers.next(self.client).await
    }
}

pager!(GetChatInviteLinkJoiners, ChatJoiner);
