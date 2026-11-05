use crate::Client;
use crate::errors::Result;
use crate::types::PeerRef;

impl Client {
    /// Closes a forum topic so only admins can post in it.
    pub async fn close_forum_topic(&self, chat_id: impl Into<PeerRef>, topic_id: i32) -> Result<()> {
        self.edit_forum_topic(chat_id, topic_id).closed(true).await
    }
}
