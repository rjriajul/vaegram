use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};

impl Client {
    /// Restores access a lapsed Star subscription already paid for, without paying again.
    pub async fn reuse_star_subscription(&self, subscription_id: &str) -> Result<()> {
        if subscription_id.is_empty() {
            return Err(Error::InvalidArgument("subscription_id is empty"));
        }
        let request = functions::payments::FulfillStarsSubscription {
            peer: base::InputPeer::InputPeerSelf,
            subscription_id: subscription_id.into(),
        };
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
    async fn sends_the_subscription_id() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes())]);
        client.reuse_star_subscription("sub1").await.unwrap();
        let request = functions::payments::FulfillStarsSubscription {
            peer: base::InputPeer::InputPeerSelf,
            subscription_id: "sub1".into(),
        };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
    }

    #[tokio::test]
    async fn an_empty_id_is_refused_without_a_request() {
        let (client, mut sent) = Client::scripted(Vec::new());
        assert!(matches!(client.reuse_star_subscription("").await, Err(Error::InvalidArgument(_))));
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn rpc_errors_come_back_unchanged() {
        let (client, _sent) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, "SUBSCRIPTION_ID_INVALID")))]);
        let result = client.reuse_star_subscription("sub1").await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "SUBSCRIPTION_ID_INVALID"), "{result:?}");
    }
}
