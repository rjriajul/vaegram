use crate::Client;
use crate::errors::Result;
use crate::types::PeerRef;

impl Client {
    pub async fn ban_chat_sender_chat(
        &self,
        chat_id: impl Into<PeerRef>,
        sender_chat_id: impl Into<PeerRef>,
    ) -> Result<()> {
        self.ban_chat_member(chat_id, sender_chat_id).await
    }
}
