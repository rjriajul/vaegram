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
        let request = functions::messages::DeleteParticipantReactions {
            peer: self.resolve_peer(chat_id).await?,
            participant: self.resolve_peer(user_id).await?,
        };
        self.invoke(&request).await?;
        Ok(())
    }
}
