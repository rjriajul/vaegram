use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn pin_chat_stories(&self, chat_id: impl Into<PeerRef>, story_ids: &[i32]) -> Result<Vec<i32>> {
        if story_ids.is_empty() {
            return Err(Error::InvalidArgument("pin_chat_stories needs at least one story id"));
        }
        let peer = self.resolve_peer(chat_id).await?;
        self.invoke(&functions::stories::TogglePinned { peer, id: story_ids.to_vec(), pinned: true }).await
    }
}
