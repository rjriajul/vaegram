use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Pins these owned gifts to the top of a profile, replacing the pinned ones; an empty list unpins all.
    pub async fn set_pinned_gifts(&self, chat_id: impl Into<PeerRef>, owned_gift_ids: &[&str]) -> Result<()> {
        let stargift = self.input_saved_gifts(owned_gift_ids).await?;
        let peer = self.resolve_peer(chat_id).await?;
        self.invoke(&functions::payments::ToggleStarGiftsPinnedToTop { peer, stargift }).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn pins_the_gifts_in_order() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes())]);
        client.set_pinned_gifts("me", &["9", "PlushPepe-3"]).await.unwrap();
        let stargift = vec![
            base::InputSavedStarGift::InputSavedStarGiftUser(types::InputSavedStarGiftUser { msg_id: 9 }),
            base::InputSavedStarGift::InputSavedStarGiftSlug(Box::new(types::InputSavedStarGiftSlug {
                slug: "PlushPepe-3".into(),
            })),
        ];
        let request =
            functions::payments::ToggleStarGiftsPinnedToTop { peer: base::InputPeer::InputPeerSelf, stargift };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
    }
}
