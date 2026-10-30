use crate::Client;
use crate::errors::Result;
use crate::types::PeerRef;

impl Client {
    pub async fn mute_chat(&self, chat_id: impl Into<PeerRef>) -> Result<bool> {
        self.update_chat_notifications(chat_id).mute_until(i32::MAX).await
    }
}
