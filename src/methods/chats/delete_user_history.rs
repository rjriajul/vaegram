use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Deletes every message a user sent in a supergroup or channel.
    ///
    /// Fails if `chat_id` is not a channel or supergroup.
    pub async fn delete_user_history(&self, chat_id: impl Into<PeerRef>, user_id: impl Into<PeerRef>) -> Result<()> {
        let chat_id = self.peer_id(chat_id.into()).await?;
        let channel = self.resolve_channel(&chat_id.into()).await?;
        let participant = self.resolve_peer(user_id).await?;
        self.affected_history(chat_id, &functions::channels::DeleteParticipantHistory { channel, participant }).await?;
        self.0.caches.evict_chat(chat_id);
        Ok(())
    }
}
