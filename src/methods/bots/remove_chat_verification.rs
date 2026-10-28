use crate::Client;
use crate::errors::Result;
use crate::types::PeerRef;

impl Client {
    pub async fn remove_chat_verification(&self, chat_id: impl Into<PeerRef>) -> Result<bool> {
        self.set_custom_verification(chat_id.into(), false, None).await
    }
}
