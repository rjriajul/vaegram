use crate::Client;
use crate::errors::Result;
use crate::types::PeerRef;

impl Client {
    pub async fn unmute_chat(&self, chat_id: impl Into<PeerRef>) -> Result<bool> {
        self.update_chat_notifications(chat_id).mute_until(0).await
    }
}
