use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Turns sponsored messages off (`restricted`) or back on for a channel you own.
    pub async fn restrict_sponsored_messages(&self, chat_id: impl Into<PeerRef>, restricted: bool) -> Result<()> {
        let channel = self.resolve_channel(&chat_id.into()).await?;
        self.invoke(&functions::channels::RestrictSponsoredMessages { channel, restricted }).await?;
        Ok(())
    }
}
