use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{ChatInviteLink, PeerRef};

impl Client {
    /// Replaces the chat's primary invite link with a new one and returns it.
    pub async fn export_chat_invite_link(&self, chat_id: impl Into<PeerRef>) -> Result<String> {
        let request = functions::messages::ExportChatInvite {
            legacy_revoke_permanent: true,
            request_needed: false,
            peer: self.resolve_peer(chat_id).await?,
            expire_date: None,
            usage_limit: None,
            title: None,
            subscription_pricing: None,
        };
        let link = ChatInviteLink::parse(self.invoke(&request).await?, &Entities::default())?;
        Ok(link.invite_link)
    }
}
