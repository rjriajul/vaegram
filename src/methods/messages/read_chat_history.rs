use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::session::Affected;
use crate::types::PeerRef;
use crate::utils::CHANNEL_OFFSET;

impl Client {
    pub async fn read_chat_history(&self, chat_id: impl Into<PeerRef>, max_id: i32) -> Result<()> {
        let chat_id = self.peer_id(chat_id.into()).await?;
        if chat_id <= -CHANNEL_OFFSET {
            let channel = self.resolve_channel(&chat_id.into()).await?;
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
