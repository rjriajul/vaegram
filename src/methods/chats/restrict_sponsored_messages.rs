use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Turns sponsored messages in a channel off, or back on when `restricted` is false.
    ///
    /// Only broadcast channels have sponsored messages, and turning them off needs the boost level in
    /// `channel_restrict_sponsored_level_min`; for a supergroup Telegram answers `CHANNEL_INVALID`.
    pub async fn restrict_sponsored_messages(&self, chat_id: impl Into<PeerRef>, restricted: bool) -> Result<()> {
        let channel = self.resolve_channel(&chat_id.into()).await?;
        self.invoke(&functions::channels::RestrictSponsoredMessages { channel, restricted }).await?;
        Ok(())
    }
}
