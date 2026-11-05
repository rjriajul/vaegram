use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Marks all unread reactions to your messages in a chat as seen.
    pub async fn read_reactions(&self, chat_id: impl Into<PeerRef>) -> Result<()> {
        let (chat_id, peer) = self.resolve(&chat_id.into()).await?;
        let request = functions::messages::ReadReactions { peer, top_msg_id: None, saved_peer_id: None };
        self.affected_history(chat_id, &request).await
    }
}
