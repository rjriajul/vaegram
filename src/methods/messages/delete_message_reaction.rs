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
        let request = functions::messages::DeleteParticipantReaction {
            peer: self.resolve_peer(chat_id).await?,
            msg_id: message_id,
            participant: self.resolve_peer(user_id).await?,
        };
        self.invoke(&request).await?;
        Ok(())
    }
}
