use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::PeerRef;

impl Client {
    pub async fn get_chat_online_count(&self, chat_id: impl Into<PeerRef>) -> Result<i32> {
        let peer = self.resolve_peer(chat_id).await?;
        let base::ChatOnlines::ChatOnlines(online) = self.invoke(&functions::messages::GetOnlines { peer }).await?;
        Ok(online.onlines)
    }
}
