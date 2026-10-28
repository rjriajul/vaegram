use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::session::Affected;
use crate::types::PeerRef;
use crate::utils::{CHANNEL_OFFSET, channel_id_of};

impl Client {
    pub async fn delete_messages(&self, chat_id: impl Into<PeerRef>, message_ids: &[i32]) -> Result<()> {
        if !(1..=100).contains(&message_ids.len()) {
            return Err(Error::InvalidArgument("deletes take 1 to 100 message ids"));
        }
        let chat_id = self.peer_id(chat_id.into()).await?;
        let id = message_ids.to_vec();
        let (channel_id, affected) = if chat_id <= -CHANNEL_OFFSET {
            let channel = self.resolve_channel(&chat_id.into()).await?;
            (Some(channel_id_of(chat_id)), self.invoke(&functions::channels::DeleteMessages { channel, id }).await?)
        } else {
            (None, self.invoke(&functions::messages::DeleteMessages { revoke: true, id }).await?)
        };
        let base::messages::AffectedMessages::AffectedMessages(a) = affected;
        self.0.events.affected(Affected { channel_id, pts: a.pts, pts_count: a.pts_count });
        Ok(())
    }
}
