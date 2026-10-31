use super::get_chat_join_requests::importers;
use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{ChatJoiner, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct GetChatInviteLinkJoiners<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    invite_link: String,
    limit: usize,
}

impl Client {
    /// Fetches every page before returning; `limit` caps it. User accounts only.
    pub fn get_chat_invite_link_joiners(
        &self,
        chat_id: impl Into<PeerRef>,
        invite_link: impl Into<String>,
    ) -> GetChatInviteLinkJoiners<'_> {
        let (chat_id, invite_link) = (chat_id.into(), invite_link.into());
        GetChatInviteLinkJoiners { client: self, chat_id, invite_link, limit: usize::MAX }
    }
}

impl GetChatInviteLinkJoiners<'_> {
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    async fn send(self) -> Result<Vec<ChatJoiner>> {
        let request = functions::messages::GetChatInviteImporters {
            requested: false,
            subscription_expired: false,
            peer: self.client.resolve_peer(self.chat_id).await?,
            link: Some(self.invite_link),
            q: None,
            offset_date: 0,
            offset_user: base::InputUser::InputUserEmpty,
            limit: 0,
        };
        importers(self.client, request, self.limit).await
    }
}

into_future!(GetChatInviteLinkJoiners, Vec<ChatJoiner>);
