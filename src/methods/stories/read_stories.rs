use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn read_stories(&self, chat_id: impl Into<PeerRef>, max_id: i32) -> Result<Vec<i32>> {
        let peer = self.resolve_peer(chat_id).await?;
        self.invoke(&functions::stories::ReadStories { peer, max_id }).await
    }
}
