use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn toggle_join_request(&self, chat_id: impl Into<PeerRef>, enabled: bool) -> Result<()> {
        let channel = self.resolve_channel(&chat_id.into()).await?;
        let request =
            functions::channels::ToggleJoinRequest { apply_to_invites: false, channel, enabled, guard_bot: None };
        self.invoke(&request).await?;
        Ok(())
    }
}
