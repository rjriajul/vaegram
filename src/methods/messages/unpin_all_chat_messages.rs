use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::session::Affected;
use crate::utils::CHANNEL_OFFSET;

impl Client {
    pub async fn unpin_all_chat_messages(&self, chat_id: i64) -> Result<()> {
        let peer = self.resolve_peer(chat_id).await?;
        loop {
            let request =
                functions::messages::UnpinAllMessages { peer: peer.clone(), top_msg_id: None, saved_peer_id: None };
            let base::messages::AffectedHistory::AffectedHistory(affected) = self.invoke(&request).await?;
            // Channels keep their own pts, which this answer doesn't move.
            if chat_id > -CHANNEL_OFFSET {
                let (pts, pts_count) = (affected.pts, affected.pts_count);
                self.0.events.affected(Affected { channel_id: None, pts, pts_count });
            }
            // Telegram unpins in batches and returns a non-zero offset until it is done.
            if affected.offset == 0 {
                return Ok(());
            }
        }
    }
}
