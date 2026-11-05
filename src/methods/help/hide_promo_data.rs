use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Hides the promoted chat returned by [`get_promo_data`](Client::get_promo_data).
    pub async fn hide_promo_data(&self, chat_id: impl Into<PeerRef>) -> Result<bool> {
        let peer = self.resolve_peer(chat_id).await?;
        self.invoke(&functions::help::HidePromoData { peer }).await
    }
}
