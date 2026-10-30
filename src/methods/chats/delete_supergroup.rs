use crate::Client;
use crate::errors::Result;
use crate::types::PeerRef;

impl Client {
    pub async fn delete_supergroup(&self, chat_id: impl Into<PeerRef>) -> Result<()> {
        self.delete_channel(chat_id).await
    }
}
