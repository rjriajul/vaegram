use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn view_stories(&self, chat_id: impl Into<PeerRef>, story_ids: &[i32]) -> Result<()> {
        if story_ids.is_empty() {
            return Err(Error::InvalidArgument("view_stories needs at least one story id"));
        }
        let peer = self.resolve_peer(chat_id).await?;
        self.invoke(&functions::stories::IncrementStoryViews { peer, id: story_ids.to_vec() }).await?;
        Ok(())
    }
}
