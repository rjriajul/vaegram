use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::PeerRef;

impl Client {
    pub async fn get_chat_history_count(&self, chat_id: impl Into<PeerRef>) -> Result<i32> {
        let request = functions::messages::GetHistory {
            peer: self.resolve_peer(chat_id).await?,
            offset_id: 0,
            offset_date: 0,
            add_offset: 0,
            limit: 1,
            max_id: 0,
            min_id: 0,
            hash: 0,
        };
        Ok(match self.invoke(&request).await? {
            base::messages::Messages::Messages(m) => m.messages.len() as i32,
            base::messages::Messages::MessagesSlice(m) => m.count,
            base::messages::Messages::ChannelMessages(m) => m.count,
            base::messages::Messages::MessagesNotModified(m) => m.count,
        })
    }
}
