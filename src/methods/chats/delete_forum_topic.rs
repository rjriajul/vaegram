use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn delete_forum_topic(&self, chat_id: impl Into<PeerRef>, topic_id: i32) -> Result<()> {
        let (chat_id, peer) = self.resolve(&chat_id.into()).await?;
        self.affected_history(chat_id, &functions::messages::DeleteTopicHistory { peer, top_msg_id: topic_id }).await?;
        self.0.caches.evict_chat(chat_id);
        self.0.caches.evict_topic(chat_id, topic_id);
        Ok(())
    }
}
