use crate::Client;
use crate::errors::Result;
use crate::types::PeerRef;

impl Client {
    /// Declines the pending request of `user_id` to join the chat.
    pub async fn decline_chat_join_request(
        &self,
        chat_id: impl Into<PeerRef>,
        user_id: impl Into<PeerRef>,
    ) -> Result<()> {
        self.hide_chat_join_request(chat_id.into(), user_id.into(), false).await
    }
}
