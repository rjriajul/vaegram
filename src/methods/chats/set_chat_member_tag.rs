use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn set_chat_member_tag(
        &self,
        chat_id: impl Into<PeerRef>,
        user_id: impl Into<PeerRef>,
        tag: impl Into<String>,
    ) -> Result<()> {
        let request = functions::messages::EditChatParticipantRank {
            peer: self.resolve_peer(chat_id).await?,
            participant: self.resolve_peer(user_id).await?,
            rank: tag.into(),
        };
        self.invoke(&request).await?;
        Ok(())
    }
}
