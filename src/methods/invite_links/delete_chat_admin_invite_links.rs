use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Deletes every revoked link the admin created.
    pub async fn delete_chat_admin_invite_links(
        &self,
        chat_id: impl Into<PeerRef>,
        admin_id: impl Into<PeerRef>,
    ) -> Result<()> {
        let request = functions::messages::DeleteRevokedExportedChatInvites {
            peer: self.resolve_peer(chat_id).await?,
            admin_id: self.resolve_user(&admin_id.into()).await?,
        };
        self.invoke(&request).await?;
        Ok(())
    }
}
