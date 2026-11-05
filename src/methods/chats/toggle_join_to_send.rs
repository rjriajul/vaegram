use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Requires people to join a supergroup before sending messages there, or turns that off.
    pub async fn toggle_join_to_send(&self, chat_id: impl Into<PeerRef>, enabled: bool) -> Result<()> {
        let channel = self.resolve_channel(&chat_id.into()).await?;
        self.invoke(&functions::channels::ToggleJoinToSend { channel, enabled }).await?;
        Ok(())
    }
}
