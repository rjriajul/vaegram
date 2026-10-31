use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{GiftCollection, PeerRef};

impl Client {
    pub async fn create_gift_collection(
        &self,
        owner_id: impl Into<PeerRef>,
        name: &str,
        owned_gift_ids: &[&str],
    ) -> Result<GiftCollection> {
        let peer = self.resolve_peer(owner_id).await?;
        let stargift = self.input_saved_gifts(owned_gift_ids).await?;
        let request = functions::payments::CreateStarGiftCollection { peer, title: name.into(), stargift };
        Ok(GiftCollection::parse(self.invoke(&request).await?))
    }
}

#[cfg(test)]
pub(super) mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::GiftCollection;

    pub(in crate::methods::payments) fn collection() -> base::StarGiftCollection {
        let collection =
            types::StarGiftCollection { collection_id: 4, title: "Best".into(), icon: None, gifts_count: 2, hash: 0 };
        base::StarGiftCollection::StarGiftCollection(Box::new(collection))
    }

    pub(in crate::methods::payments) fn collection_reply() -> Vec<u8> {
        collection().to_bytes()
    }

    pub(in crate::methods::payments) fn best() -> GiftCollection {
        GiftCollection { id: 4, name: "Best".into(), icon: None, gift_count: 2 }
    }

    pub(in crate::methods::payments) fn gifts() -> Vec<base::InputSavedStarGift> {
        vec![
            base::InputSavedStarGift::InputSavedStarGiftUser(types::InputSavedStarGiftUser { msg_id: 9 }),
            base::InputSavedStarGift::InputSavedStarGiftSlug(Box::new(types::InputSavedStarGiftSlug {
                slug: "PlushPepe-3".into(),
            })),
        ]
    }

    #[tokio::test]
    async fn creates_a_collection() {
        let (client, mut sent) = Client::scripted(vec![Ok(collection_reply())]);
        let created = client.create_gift_collection("me", "Best", &["9", "PlushPepe-3"]).await.unwrap();
        assert_eq!(created, best());
        let request = functions::payments::CreateStarGiftCollection {
            peer: base::InputPeer::InputPeerSelf,
            title: "Best".into(),
            stargift: gifts(),
        };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
    }

    #[tokio::test]
    async fn rpc_errors_come_back_unchanged() {
        let (client, _sent) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, "COLLECTIONS_TOO_MANY")))]);
        let result = client.create_gift_collection("me", "Best", &[]).await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "COLLECTIONS_TOO_MANY"), "{result:?}");
    }
}
