use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::session::Affected;
use crate::utils::CHANNEL_OFFSET;

impl Client {
    pub async fn read_chat_history(&self, chat_id: i64, max_id: i32) -> Result<()> {
        if chat_id <= -CHANNEL_OFFSET {
            let channel = self.resolve_channel(chat_id).await?;
            self.invoke(&functions::channels::ReadHistory { channel, max_id }).await?;
        } else {
            let peer = self.resolve_peer(chat_id).await?;
            let request = functions::messages::ReadHistory { peer, max_id };
            let base::messages::AffectedMessages::AffectedMessages(a) = self.invoke(&request).await?;
            self.0.events.affected(Affected { channel_id: None, pts: a.pts, pts_count: a.pts_count });
        }
        Ok(())
    }
}
