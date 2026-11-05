use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Hides the member list of a supergroup from regular members, or shows it again.
    pub async fn toggle_participants_hidden(&self, chat_id: impl Into<PeerRef>, enabled: bool) -> Result<()> {
        let channel = self.resolve_channel(&chat_id.into()).await?;
        self.invoke(&functions::channels::ToggleParticipantsHidden { channel, enabled }).await?;
        Ok(())
    }
}
