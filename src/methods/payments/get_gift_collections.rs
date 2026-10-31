use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{GiftCollection, PeerRef};

impl Client {
    pub async fn get_gift_collections(&self, owner_id: impl Into<PeerRef>) -> Result<Vec<GiftCollection>> {
        let peer = self.resolve_peer(owner_id).await?;
        Ok(match self.invoke(&functions::payments::GetStarGiftCollections { peer, hash: 0 }).await? {
            base::payments::StarGiftCollections::StarGiftCollections(list) => {
                list.collections.into_iter().map(GiftCollection::parse).collect()
            }
            base::payments::StarGiftCollections::StarGiftCollectionsNotModified => Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::create_gift_collection::tests::{best, collection};
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn lists_the_collections() {
        let list = types::payments::StarGiftCollections { collections: vec![collection()] };
        let list = base::payments::StarGiftCollections::StarGiftCollections(Box::new(list));
        let (client, mut sent) = Client::scripted(vec![Ok(list.to_bytes())]);
        assert_eq!(client.get_gift_collections("me").await.unwrap(), vec![best()]);
        let request = functions::payments::GetStarGiftCollections { peer: base::InputPeer::InputPeerSelf, hash: 0 };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
    }

    #[tokio::test]
    async fn not_modified_is_an_empty_list() {
        let reply = base::payments::StarGiftCollections::StarGiftCollectionsNotModified.to_bytes();
        let (client, _sent) = Client::scripted(vec![Ok(reply)]);
        assert!(client.get_gift_collections("me").await.unwrap().is_empty());
    }
}
