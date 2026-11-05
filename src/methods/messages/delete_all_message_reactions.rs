use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Removes up to 10000 recent reactions a user or chat (`user_id` may be either) left in a group.
    pub async fn delete_all_message_reactions(
        &self,
        chat_id: impl Into<PeerRef>,
        user_id: impl Into<PeerRef>,
    ) -> Result<()> {
        let (chat_id, peer) = self.resolve(&chat_id.into()).await?;
        let request =
            functions::messages::DeleteParticipantReactions { peer, participant: self.resolve_peer(user_id).await? };
        self.invoke(&request).await?;
        // Any message of the chat may have lost a reaction.
        self.0.caches.evict_chat(chat_id);
        Ok(())
    }
}
