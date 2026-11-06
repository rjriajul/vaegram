use crate::Client;
use crate::errors::Result;
use crate::types::PeerRef;

impl Client {
    /// Removes the verification your organization gave to a chat.
    pub async fn remove_chat_verification(&self, chat_id: impl Into<PeerRef>) -> Result<()> {
        self.set_custom_verification(chat_id.into(), false, None).await
    }
}
