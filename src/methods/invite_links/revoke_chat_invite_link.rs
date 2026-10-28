use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{ChatInviteLink, PeerRef};

impl Client {
    /// Returns the revoked link. Revoking the primary link also creates a new one.
    pub async fn revoke_chat_invite_link(
        &self,
        chat_id: impl Into<PeerRef>,
        invite_link: impl Into<String>,
    ) -> Result<ChatInviteLink> {
        let request = functions::messages::EditExportedChatInvite {
            revoked: true,
            peer: self.resolve_peer(chat_id).await?,
            link: invite_link.into(),
            expire_date: None,
            usage_limit: None,
            request_needed: None,
            title: None,
        };
        ChatInviteLink::parse_edited(self, self.invoke(&request).await?)
    }
}
