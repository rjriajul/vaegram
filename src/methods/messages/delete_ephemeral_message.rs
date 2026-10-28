use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn delete_ephemeral_message(
        &self,
        chat_id: impl Into<PeerRef>,
        receiver_user_id: impl Into<PeerRef>,
        ephemeral_message_id: i32,
    ) -> Result<bool> {
        let request = functions::ephemeral::DeleteMessage {
            peer: Some(self.resolve(&chat_id.into()).await?.1),
            receiver_id: self.resolve_user(&receiver_user_id.into()).await?,
            id: ephemeral_message_id,
        };
        self.invoke(&request).await
    }
}
