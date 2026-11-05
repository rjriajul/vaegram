use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Turns automatic translation of posts on or off for a channel.
    pub async fn toggle_auto_translation(&self, chat_id: impl Into<PeerRef>, enabled: bool) -> Result<()> {
        let channel = self.resolve_channel(&chat_id.into()).await?;
        self.invoke(&functions::channels::ToggleAutotranslation { channel, enabled }).await?;
        Ok(())
    }
}
