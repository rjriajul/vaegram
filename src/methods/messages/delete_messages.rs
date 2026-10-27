use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};

impl Client {
    pub async fn delete_messages(&self, chat_id: i64, message_ids: &[i32]) -> Result<()> {
        let id = message_ids.to_vec();
        match self.resolve_peer(chat_id).await? {
            base::InputPeer::InputPeerChannel(p) => {
                let channel = types::InputChannel { channel_id: p.channel_id, access_hash: p.access_hash };
                let channel = base::InputChannel::InputChannel(Box::new(channel));
                self.invoke(&functions::channels::DeleteMessages { channel, id }).await?;
            }
            _ => {
                self.invoke(&functions::messages::DeleteMessages { revoke: true, id }).await?;
            }
        }
        Ok(())
    }
}
