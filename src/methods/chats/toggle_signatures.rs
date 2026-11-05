use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Adds the author's name to channel posts, or removes it. `profiles` also links the author's profile.
    pub async fn toggle_signatures(&self, chat_id: impl Into<PeerRef>, enabled: bool, profiles: bool) -> Result<()> {
        let channel = self.resolve_channel(&chat_id.into()).await?;
        let request =
            functions::channels::ToggleSignatures { signatures_enabled: enabled, profiles_enabled: profiles, channel };
        self.invoke(&request).await?;
        Ok(())
    }
}
