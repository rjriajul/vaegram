use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Deletes the collection only; its gifts stay on the profile.
    pub async fn delete_gift_collection(&self, owner_id: impl Into<PeerRef>, collection_id: i32) -> Result<()> {
        let peer = self.resolve_peer(owner_id).await?;
        self.invoke(&functions::payments::DeleteStarGiftCollection { peer, collection_id }).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions};

    #[tokio::test]
    async fn deletes_the_collection() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes())]);
        client.delete_gift_collection("me", 4).await.unwrap();
        let request =
            functions::payments::DeleteStarGiftCollection { peer: base::InputPeer::InputPeerSelf, collection_id: 4 };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
    }
}
