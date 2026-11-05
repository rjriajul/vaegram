use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Sets the auto-delete timer for new messages in a chat. 0 turns it off.
    pub async fn set_chat_ttl(&self, chat_id: impl Into<PeerRef>, ttl_seconds: i32) -> Result<()> {
        let peer = self.resolve_peer(chat_id).await?;
        self.invoke(&functions::messages::SetHistoryTTL { peer, period: ttl_seconds }).await?;
        Ok(())
    }
}
