use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn delete_forum_topic(&self, chat_id: impl Into<PeerRef>, topic_id: i32) -> Result<()> {
        let (chat_id, peer) = self.resolve(&chat_id.into()).await?;
        self.affected_history(chat_id, &functions::messages::DeleteTopicHistory { peer, top_msg_id: topic_id }).await
    }
}
