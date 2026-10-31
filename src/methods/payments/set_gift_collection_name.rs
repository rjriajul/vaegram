use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{GiftCollection, PeerRef};

impl Client {
    pub async fn set_gift_collection_name(
        &self,
        owner_id: impl Into<PeerRef>,
        collection_id: i32,
        name: &str,
    ) -> Result<GiftCollection> {
        let request = functions::payments::UpdateStarGiftCollection {
            peer: self.resolve_peer(owner_id).await?,
            collection_id,
            title: Some(name.into()),
            delete_stargift: None,
            add_stargift: None,
            order: None,
        };
        Ok(GiftCollection::parse(self.invoke(&request).await?))
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::create_gift_collection::tests::{best, collection_reply};
    use crate::Client;
    use crate::raw::{Serializable, base, functions};

    pub(in crate::methods::payments) fn update(
        title: Option<&str>,
        delete_stargift: Option<Vec<base::InputSavedStarGift>>,
        add_stargift: Option<Vec<base::InputSavedStarGift>>,
        order: Option<Vec<base::InputSavedStarGift>>,
    ) -> Vec<u8> {
        let request = functions::payments::UpdateStarGiftCollection {
            peer: base::InputPeer::InputPeerSelf,
            collection_id: 4,
            title: title.map(Into::into),
            delete_stargift,
            add_stargift,
            order,
        };
        request.to_bytes()
    }

    #[tokio::test]
    async fn renames_only() {
        let (client, mut sent) = Client::scripted(vec![Ok(collection_reply())]);
        assert_eq!(client.set_gift_collection_name("me", 4, "Best").await.unwrap(), best());
        assert_eq!(sent.try_recv().unwrap(), update(Some("Best"), None, None, None));
    }
}
