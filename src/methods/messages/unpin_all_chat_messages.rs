use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};

impl Client {
    pub async fn unpin_all_chat_messages(&self, chat_id: i64) -> Result<()> {
        let peer = self.resolve_peer(chat_id).await?;
        loop {
            let request =
                functions::messages::UnpinAllMessages { peer: peer.clone(), top_msg_id: None, saved_peer_id: None };
            let base::messages::AffectedHistory::AffectedHistory(affected) = self.invoke(&request).await?;
            // Telegram unpins in batches and returns a non-zero offset until it is done.
            if affected.offset == 0 {
                return Ok(());
            }
        }
    }
}
