use crate::Client;
use crate::errors::Result;
use crate::types::PeerRef;

impl Client {
    /// Unpins every pinned message in a forum topic.
    pub async fn unpin_all_forum_topic_messages(&self, chat_id: impl Into<PeerRef>, topic_id: i32) -> Result<()> {
        self.unpin_all(chat_id.into(), Some(topic_id)).await
    }
}
