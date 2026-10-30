use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn set_chat_protected_content(&self, chat_id: impl Into<PeerRef>, enabled: bool) -> Result<()> {
        let peer = self.resolve_peer(chat_id).await?;
        self.invoke(&functions::messages::ToggleNoForwards { peer, enabled, request_msg_id: None }).await?;
        Ok(())
    }
}
