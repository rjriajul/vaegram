use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;

impl Client {
    /// The gift is gone for good, with no Stars in return.
    pub async fn delete_gift(&self, owned_gift_id: &str) -> Result<()> {
        if owned_gift_id.is_empty() {
            return Err(Error::InvalidArgument("owned_gift_id is empty"));
        }
        let stargift = self.input_saved_gift(owned_gift_id).await?;
        self.invoke(&functions::payments::DeleteStarGift { stargift }).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::{Serializable, base, functions, types};

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
}
