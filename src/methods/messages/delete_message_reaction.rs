use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Removes the reaction a user or chat (`user_id` may be either) left on a message in a group.
    pub async fn delete_message_reaction(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
        user_id: impl Into<PeerRef>,
    ) -> Result<()> {
        let (chat_id, peer) = self.resolve(&chat_id.into()).await?;
        let request = functions::messages::DeleteParticipantReaction {
            peer,
            msg_id: message_id,
            participant: self.resolve_peer(user_id).await?,
        };
        self.invoke(&request).await?;
        self.0.caches.evict(chat_id, &[message_id]);
        Ok(())
    }
}
