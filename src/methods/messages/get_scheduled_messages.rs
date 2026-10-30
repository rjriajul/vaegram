use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{Message, PeerRef};

impl Client {
    pub async fn get_scheduled_messages(&self, chat_id: impl Into<PeerRef>) -> Result<Vec<Message>> {
        let peer = self.resolve_peer(chat_id).await?;
        let found = self.invoke(&functions::messages::GetScheduledHistory { peer, hash: 0 }).await?;
        let (messages, entities) = Message::unpack(self, found);
        Ok(messages.into_iter().filter_map(|m| Message::from_raw(self, m, &entities)).collect())
    }
}
