use crate::Client;
use crate::errors::Result;
use crate::types::PeerRef;

impl Client {
    pub async fn unpin_forum_topic(&self, chat_id: impl Into<PeerRef>, topic_id: i32) -> Result<()> {
        self.toggle_forum_topic_pin(chat_id, topic_id, false).await
    }
}
