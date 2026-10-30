use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn disable_peer_connected_bot(&self, chat_id: impl Into<PeerRef>) -> Result<bool> {
        let peer = self.resolve_peer(chat_id).await?;
        self.invoke(&functions::account::DisablePeerConnectedBot { peer }).await
    }
}
