use crate::Client;
use crate::errors::Result;
use crate::types::PeerRef;

impl Client {
    pub async fn unpin_saved_dialog(&self, chat_id: impl Into<PeerRef>) -> Result<()> {
        self.toggle_saved_dialog_pin(chat_id, false).await
    }
}
