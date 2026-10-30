use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn delete_scheduled_messages(&self, chat_id: impl Into<PeerRef>, message_ids: &[i32]) -> Result<()> {
        if message_ids.is_empty() {
            return Err(Error::InvalidArgument("delete_scheduled_messages needs at least one message id"));
        }
        let peer = self.resolve_peer(chat_id).await?;
        self.invoke(&functions::messages::DeleteScheduledMessages { peer, id: message_ids.to_vec() }).await?;
        Ok(())
    }
}
