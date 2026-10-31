use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn pin_forum_topic(&self, chat_id: impl Into<PeerRef>, topic_id: i32) -> Result<()> {
        self.toggle_forum_topic_pin(chat_id, topic_id, true).await
    }

    pub(crate) async fn toggle_forum_topic_pin(
        &self,
        chat_id: impl Into<PeerRef>,
        topic_id: i32,
        pinned: bool,
    ) -> Result<()> {
        let peer = self.resolve_peer(chat_id).await?;
        self.invoke(&functions::messages::UpdatePinnedForumTopic { peer, topic_id, pinned }).await?;
        Ok(())
    }
}
