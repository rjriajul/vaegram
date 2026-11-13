use crate::Client;
use crate::errors::Result;

impl Client {
    /// Deletes a regular gift owned by a managed business account, with no Stars in return.
    ///
    /// Same call as [`Client::delete_gift`] with its `business_connection_id` set, which is the shape Bot API 10.4
    /// gave it. `owned_gift_id` is the `owned_gift_id` of an owned gift, or a unique gift's slug. Requires the
    /// `can_convert_gifts_to_stars` business bot right; Telegram refuses a gift that is limited or was refunded.
    pub async fn delete_business_account_gift(
        &self,
        business_connection_id: impl Into<String>,
        owned_gift_id: impl Into<String>,
    ) -> Result<()> {
        self.delete_gift(owned_gift_id).business_connection_id(business_connection_id).await
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Identifiable, Serializable, base, functions, types};

    #[tokio::test]
    async fn deletes_the_gift_of_the_business_account() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes())]);
        client.0.caches.know_business("conn", 7, 2);
        client.delete_business_account_gift("conn", "PlushPepe-3").await.unwrap();
        let body = sent.try_recv().unwrap();
        let wrapper = functions::InvokeWithBusinessConnection::<functions::payments::DeleteStarGift>::CONSTRUCTOR_ID;
        assert_eq!(body[..4], wrapper.to_le_bytes());
        let stargift = base::InputSavedStarGift::InputSavedStarGiftSlug(Box::new(types::InputSavedStarGiftSlug {
            slug: "PlushPepe-3".into(),
        }));
        let inner = functions::payments::DeleteStarGift { stargift }.to_bytes();
        assert!(body.windows(inner.len()).any(|w| w == inner), "the delete request rides inside the wrapper");
        let connection_id = "conn".to_owned().to_bytes();
        assert!(body.windows(connection_id.len()).any(|w| w == connection_id), "the wrapper carries the connection id");
    }

    #[tokio::test]
    async fn an_empty_id_is_refused_without_a_request() {
        let (client, mut sent) = Client::scripted(Vec::new());
        let refused = client.delete_business_account_gift("conn", "").await;
        assert!(matches!(&refused, Err(Error::InvalidArgument(_))), "{refused:?}");
        assert!(sent.try_recv().is_err());
    }
}
