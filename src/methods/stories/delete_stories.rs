use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Deletes stories of a chat and returns the ids that were deleted.
    ///
    /// Fails with `Error::InvalidArgument` if `story_ids` is empty.
    pub async fn delete_stories(&self, chat_id: impl Into<PeerRef>, story_ids: &[i32]) -> Result<Vec<i32>> {
        if story_ids.is_empty() {
            return Err(Error::InvalidArgument("delete_stories needs at least one story id"));
        }
        let peer = self.resolve_peer(chat_id).await?;
        self.invoke(&functions::stories::DeleteStories { peer, id: story_ids.to_vec() }).await
    }
}
