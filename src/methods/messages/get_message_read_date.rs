use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::PeerRef;

impl Client {
    pub async fn get_message_read_date(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> Result<Option<i32>> {
        let request =
            functions::messages::GetOutboxReadDate { peer: self.resolve_peer(chat_id).await?, msg_id: message_id };
        match self.invoke(&request).await {
            Ok(base::OutboxReadDate::OutboxReadDate(read)) => Ok(Some(read.date)),
            Err(e) if e.is("MESSAGE_NOT_READ_YET") => Ok(None),
            Err(e) => Err(e),
        }
    }
}
