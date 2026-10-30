use crate::Client;
use crate::errors::Result;
use crate::raw::base::ReadParticipantDate;
use crate::raw::functions;
use crate::types::{PeerRef, ReadParticipant};

impl Client {
    /// Only in groups under the app config's `chat_read_mark_size_threshold` members, and only for
    /// `chat_read_mark_expire_period` seconds after the message was sent.
    pub async fn get_message_read_participants(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
    ) -> Result<Vec<ReadParticipant>> {
        let peer = self.resolve_peer(chat_id).await?;
        let read = self.invoke(&functions::messages::GetMessageReadParticipants { peer, msg_id: message_id }).await?;
        Ok(read
            .into_iter()
            .map(|ReadParticipantDate::ReadParticipantDate(r)| ReadParticipant { user_id: r.user_id, date: r.date })
            .collect())
    }
}
