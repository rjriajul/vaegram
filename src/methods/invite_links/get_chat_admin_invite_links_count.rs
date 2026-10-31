use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::PeerRef;

impl Client {
    pub async fn get_chat_admin_invite_links_count(
        &self,
        chat_id: impl Into<PeerRef>,
        admin_id: impl Into<PeerRef>,
        revoked: bool,
    ) -> Result<i32> {
        let request = functions::messages::GetExportedChatInvites {
            revoked,
            peer: self.resolve_peer(chat_id).await?,
            admin_id: self.resolve_user(&admin_id.into()).await?,
            offset_date: None,
            offset_link: None,
            limit: 1,
        };
        let base::messages::ExportedChatInvites::ExportedChatInvites(page) = self.invoke(&request).await?;
        Ok(page.count)
    }
}
