use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Accepts or declines the purchase offer in message `message_id`; accepting sells the gift.
    pub async fn process_gift_purchase_offer(&self, message_id: i32, accept: bool) -> Result<()> {
        let request = functions::payments::ResolveStarGiftOffer { decline: !accept, offer_msg_id: message_id };
        self.invoke(&request).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::{Serializable, base, functions};

    #[tokio::test]
    async fn accepts_and_declines() {
        let replies = (0..2).map(|_| Ok(base::Updates::UpdatesTooLong.to_bytes())).collect();
        let (client, mut sent) = Client::scripted(replies);
        client.process_gift_purchase_offer(12, true).await.unwrap();
        client.process_gift_purchase_offer(12, false).await.unwrap();
        for decline in [false, true] {
            let request = functions::payments::ResolveStarGiftOffer { decline, offer_msg_id: 12 };
            assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
        }
    }

    #[tokio::test]
    async fn rpc_errors_come_back_unchanged() {
        let (client, _sent) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, "MESSAGE_ID_INVALID")))]);
        let result = client.process_gift_purchase_offer(12, true).await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "MESSAGE_ID_INVALID"), "{result:?}");
    }
}
