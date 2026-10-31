use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn set_chat_ttl(&self, chat_id: impl Into<PeerRef>, ttl_seconds: i32) -> Result<()> {
        let peer = self.resolve_peer(chat_id).await?;
        self.invoke(&functions::messages::SetHistoryTTL { peer, period: ttl_seconds }).await?;
        Ok(())
    }
}
