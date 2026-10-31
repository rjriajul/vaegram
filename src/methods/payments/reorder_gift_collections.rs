use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn reorder_gift_collections(&self, owner_id: impl Into<PeerRef>, collection_ids: &[i32]) -> Result<()> {
        if collection_ids.is_empty() {
            return Err(Error::InvalidArgument("collection_ids is empty"));
        }
        let peer = self.resolve_peer(owner_id).await?;
        let request = functions::payments::ReorderStarGiftCollections { peer, order: collection_ids.to_vec() };
        self.invoke(&request).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions};

    #[tokio::test]
    async fn sends_the_order() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes())]);
        client.reorder_gift_collections("me", &[4, 2]).await.unwrap();
        let request =
            functions::payments::ReorderStarGiftCollections { peer: base::InputPeer::InputPeerSelf, order: vec![4, 2] };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
    }

    #[tokio::test]
    async fn an_empty_order_is_refused_without_a_request() {
        let (client, mut sent) = Client::scripted(Vec::new());
        assert!(matches!(client.reorder_gift_collections("me", &[]).await, Err(Error::InvalidArgument(_))));
        assert!(sent.try_recv().is_err());
    }
}
