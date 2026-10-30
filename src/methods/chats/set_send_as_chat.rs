use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn set_send_as_chat(
        &self,
        chat_id: impl Into<PeerRef>,
        send_as_chat_id: impl Into<PeerRef>,
    ) -> Result<()> {
        let peer = self.resolve_peer(chat_id).await?;
        let send_as = self.resolve_peer(send_as_chat_id).await?;
        self.invoke(&functions::messages::SaveDefaultSendAs { peer, send_as }).await?;
        Ok(())
    }
}
