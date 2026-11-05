use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;

/// Request built by [`Client::delete_gift`].
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteGift<'a> {
    client: &'a Client,
    owned_gift_id: String,
    connection_id: Option<String>,
}

impl Client {
    /// Deletes a received gift for good, with no Stars in return.
    ///
    /// `owned_gift_id` is the `owned_gift_id` of an owned gift, or a unique gift's slug. Fails with
    /// `InvalidArgument` if it is empty.
    pub fn delete_gift(&self, owned_gift_id: impl Into<String>) -> DeleteGift<'_> {
        DeleteGift { client: self, owned_gift_id: owned_gift_id.into(), connection_id: None }
    }
}

impl DeleteGift<'_> {
    /// Deletes the gift of the business account behind this connection.
    pub fn business_connection_id(mut self, id: impl Into<String>) -> Self {
        self.connection_id = Some(id.into());
        self
    }

    async fn send(self) -> Result<()> {
        if self.owned_gift_id.is_empty() {
            return Err(Error::InvalidArgument("owned_gift_id is empty"));
        }
        let stargift = self.client.input_saved_gift(&self.owned_gift_id).await?;
        let request = functions::payments::DeleteStarGift { stargift };
        self.client.invoke_business(self.connection_id.as_deref(), request).await?;
        Ok(())
    }
}

into_future!(DeleteGift, ());

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::{Identifiable, Serializable, base, functions, types};

    #[tokio::test]
    async fn deletes_the_gift() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes())]);
        client.delete_gift("PlushPepe-3").await.unwrap();
        let stargift = base::InputSavedStarGift::InputSavedStarGiftSlug(Box::new(types::InputSavedStarGiftSlug {
            slug: "PlushPepe-3".into(),
        }));
        assert_eq!(sent.try_recv().unwrap(), functions::payments::DeleteStarGift { stargift }.to_bytes());
    }

    #[tokio::test]
    async fn an_empty_id_is_refused_without_a_request() {
        let (client, mut sent) = Client::scripted(Vec::new());
        assert!(matches!(client.delete_gift("").await, Err(Error::InvalidArgument(_))));
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn rpc_errors_come_back_unchanged() {
        let (client, _sent) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, "STARGIFT_INVALID")))]);
        let result = client.delete_gift("9").await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "STARGIFT_INVALID"), "{result:?}");
    }

    #[tokio::test]
    async fn delete_gift_on_behalf_wraps_business() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes())]);
        client.delete_gift("PlushPepe-3").business_connection_id("conn").await.unwrap();
        let body = sent.try_recv().unwrap();
        let wrapper = functions::InvokeWithBusinessConnection::<functions::payments::DeleteStarGift>::CONSTRUCTOR_ID;
        assert_eq!(body[..4], wrapper.to_le_bytes());
        let stargift = base::InputSavedStarGift::InputSavedStarGiftSlug(Box::new(types::InputSavedStarGiftSlug {
            slug: "PlushPepe-3".into(),
        }));
        let inner = functions::payments::DeleteStarGift { stargift }.to_bytes();
        assert!(body.windows(inner.len()).any(|w| w == inner), "the gift request rides inside the wrapper");
        let connection_id = "conn".to_owned().to_bytes();
        assert!(body.windows(connection_id.len()).any(|w| w == connection_id), "the wrapper carries the connection id");
    }
}
