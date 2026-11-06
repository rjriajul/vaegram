use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Stops the account's connected business bot from handling one chat.
    pub async fn disable_peer_connected_bot(&self, chat_id: impl Into<PeerRef>) -> Result<()> {
        let peer = self.resolve_peer(chat_id).await?;
        accepted(self.invoke(&functions::account::DisablePeerConnectedBot { peer }).await?)
    }
}
