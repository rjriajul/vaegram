use crate::Client;
use crate::errors::Result;
use crate::raw::{RemoteCall, base, functions};
use crate::session::Affected;
use crate::types::PeerRef;
use crate::utils::{CHANNEL_OFFSET, get_channel_id};

impl Client {
    pub async fn unpin_all_chat_messages(&self, chat_id: impl Into<PeerRef>) -> Result<()> {
        self.unpin_all(chat_id.into(), None).await
    }

    pub(crate) async fn unpin_all(&self, chat_id: PeerRef, top_msg_id: Option<i32>) -> Result<()> {
        let (chat_id, peer) = self.resolve(&chat_id).await?;
        let request = functions::messages::UnpinAllMessages { peer, top_msg_id, saved_peer_id: None };
        self.affected_history(chat_id, &request).await
    }

    pub(crate) async fn affected_history<R>(&self, chat_id: i64, request: &R) -> Result<()>
    where
        R: RemoteCall<Return = base::messages::AffectedHistory>,
    {
        // In a channel the answer counts in the channel's own pts, not the account's.
        let channel_id = (chat_id <= -CHANNEL_OFFSET).then(|| get_channel_id(chat_id));
        loop {
            let base::messages::AffectedHistory::AffectedHistory(affected) = self.invoke(request).await?;
            self.0.events.affected(Affected { channel_id, pts: affected.pts, pts_count: affected.pts_count });
            // Telegram works in batches and returns a non-zero offset until it is done.
            if affected.offset == 0 {
                return Ok(());
            }
        }
    }
}
