use crate::Client;
use crate::errors::Result;
use crate::types::PeerRef;

impl Client {
    /// Lets a banned channel post in a chat as itself again.
    ///
    /// Calls [`Client::unban_chat_member`] with the channel as the member.
    pub async fn unban_chat_sender_chat(
        &self,
        chat_id: impl Into<PeerRef>,
        sender_chat_id: impl Into<PeerRef>,
    ) -> Result<()> {
        self.unban_chat_member(chat_id, sender_chat_id).await
    }
}
