use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Pins these owned gifts to the top of a profile, replacing the pinned ones; an empty list unpins all.
    pub async fn set_pinned_gifts(&self, chat_id: impl Into<PeerRef>, owned_gift_ids: &[&str]) -> Result<()> {
        let mut stargift = Vec::with_capacity(owned_gift_ids.len());
        for id in owned_gift_ids {
            stargift.push(self.input_saved_gift(id).await?);
        }
        let peer = self.resolve_peer(chat_id).await?;
        self.invoke(&functions::payments::ToggleStarGiftsPinnedToTop { peer, stargift }).await?;
        Ok(())
    }
}
