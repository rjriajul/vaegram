use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn unpin_chat_message(&self, chat_id: i64, message_id: i32) -> Result<()> {
        let request = functions::messages::UpdatePinnedMessage {
            silent: false,
            unpin: true,
            pm_oneside: false,
            peer: self.resolve_peer(chat_id).await?,
            id: message_id,
        };
        self.invoke(&request).await?;
        Ok(())
    }
}
