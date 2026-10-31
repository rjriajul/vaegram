use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};

impl Client {
    /// Cancels a Star subscription's renewals, or turns them back on; nothing is charged right away.
    pub async fn edit_star_subscription(&self, subscription_id: &str, is_canceled: bool) -> Result<()> {
        if subscription_id.is_empty() {
            return Err(Error::InvalidArgument("subscription_id is empty"));
        }
        let request = functions::payments::ChangeStarsSubscription {
            peer: base::InputPeer::InputPeerSelf,
            subscription_id: subscription_id.into(),
            canceled: Some(is_canceled),
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
    async fn cancels_and_restores() {
        let replies = vec![Ok(true.to_bytes()), Ok(true.to_bytes())];
        let (client, mut sent) = Client::scripted(replies);
        for canceled in [true, false] {
            client.edit_star_subscription("sub1", canceled).await.unwrap();
            let request = functions::payments::ChangeStarsSubscription {
                peer: base::InputPeer::InputPeerSelf,
                subscription_id: "sub1".into(),
                canceled: Some(canceled),
            };
            assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
        }
    }

    #[tokio::test]
    async fn an_empty_id_is_refused_without_a_request() {
        let (client, mut sent) = Client::scripted(Vec::new());
        assert!(matches!(client.edit_star_subscription("", true).await, Err(Error::InvalidArgument(_))));
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn rpc_errors_come_back_unchanged() {
        let (client, _sent) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, "SUBSCRIPTION_ID_INVALID")))]);
        let result = client.edit_star_subscription("sub1", true).await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "SUBSCRIPTION_ID_INVALID"), "{result:?}");
    }
}
