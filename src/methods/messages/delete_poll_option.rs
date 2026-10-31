use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// `option` is the option's position in the poll, from 0.
    pub async fn delete_poll_option(&self, chat_id: impl Into<PeerRef>, message_id: i32, option: usize) -> Result<()> {
        let chat_id = self.peer_id(chat_id.into()).await?;
        let option = self.poll_option_ids(chat_id, message_id, &[option]).await?.swap_remove(0);
        let request = functions::messages::DeletePollAnswer {
            peer: self.resolve_peer(chat_id).await?,
            msg_id: message_id,
            option,
        };
        self.invoke(&request).await.map(drop)
    }
}
