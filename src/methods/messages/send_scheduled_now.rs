use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::{Message, PeerRef};

impl Client {
    pub async fn send_scheduled_now(&self, chat_id: impl Into<PeerRef>, message_ids: &[i32]) -> Result<Vec<Message>> {
        if message_ids.is_empty() {
            return Err(Error::InvalidArgument("send_scheduled_now needs at least one message id"));
        }
        let request = functions::messages::SendScheduledMessages {
            peer: self.resolve_peer(chat_id).await?,
            id: message_ids.to_vec(),
        };
        let updates = self.invoke(&request).await?;
        Ok(Message::all_from_updates(self, updates).collect())
    }
}
