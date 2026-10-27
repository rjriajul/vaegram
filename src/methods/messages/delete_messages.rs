use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::utils::CHANNEL_OFFSET;

impl Client {
    pub async fn delete_messages(&self, chat_id: i64, message_ids: &[i32]) -> Result<()> {
        let id = message_ids.to_vec();
        if chat_id <= -CHANNEL_OFFSET {
            let channel = self.resolve_channel(chat_id).await?;
            self.invoke(&functions::channels::DeleteMessages { channel, id }).await?;
        } else {
            self.invoke(&functions::messages::DeleteMessages { revoke: true, id }).await?;
        }
        Ok(())
    }
}
