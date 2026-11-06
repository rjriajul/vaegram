use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::{GiftCollection, PeerRef};

impl Client {
    /// The listed gifts move to the front of the collection in this order.
    pub async fn reorder_collection_gifts(
        &self,
        owner_id: impl Into<PeerRef>,
        collection_id: i32,
        owned_gift_ids: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<GiftCollection> {
        let stargift = self.input_saved_gifts(owned_gift_ids).await?;
        if stargift.is_empty() {
            return Err(Error::InvalidArgument("reorder_collection_gifts needs at least one owned gift id"));
        }
        let peer = self.resolve_peer(owner_id).await?;
        let request = functions::payments::UpdateStarGiftCollection {
            peer,
            collection_id,
            title: None,
            delete_stargift: None,
            add_stargift: None,
            order: Some(stargift),
        };
        Ok(GiftCollection::parse(self.invoke(&request).await?))
    }
}

#[cfg(test)]
mod tests {
    use super::super::create_gift_collection::tests::{best, collection_reply, gifts};
    use super::super::set_gift_collection_name::tests::update;
    use crate::Client;
    use crate::errors::Error;

    #[tokio::test]
    async fn reorders_only() {
        let (client, mut sent) = Client::scripted(vec![Ok(collection_reply())]);
        assert_eq!(client.reorder_collection_gifts("me", 4, &["9", "PlushPepe-3"]).await.unwrap(), best());
        assert_eq!(sent.try_recv().unwrap(), update(None, None, None, Some(gifts())));
    }

    #[tokio::test]
    async fn no_gifts_is_refused_without_a_request() {
        let (client, mut sent) = Client::scripted(Vec::new());
        let refused = client.reorder_collection_gifts("me", 4, Vec::<String>::new()).await;
        assert!(matches!(refused, Err(Error::InvalidArgument(_))));
        assert!(sent.try_recv().is_err());
    }
}
