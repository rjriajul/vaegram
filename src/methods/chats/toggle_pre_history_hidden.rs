use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn toggle_pre_history_hidden(&self, chat_id: impl Into<PeerRef>, enabled: bool) -> Result<()> {
        let channel = self.resolve_channel(&chat_id.into()).await?;
        self.invoke(&functions::channels::TogglePreHistoryHidden { channel, enabled }).await?;
        Ok(())
    }
}
