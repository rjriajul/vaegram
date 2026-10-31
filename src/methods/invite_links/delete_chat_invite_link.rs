use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn delete_chat_invite_link(
        &self,
        chat_id: impl Into<PeerRef>,
        invite_link: impl Into<String>,
    ) -> Result<()> {
        let request = functions::messages::DeleteExportedChatInvite {
            peer: self.resolve_peer(chat_id).await?,
            link: invite_link.into(),
        };
        self.invoke(&request).await?;
        Ok(())
    }
}
