use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::PeerRef;

impl Client {
    pub async fn invoke_web_view_custom_method(
        &self,
        bot_id: impl Into<PeerRef>,
        method: &str,
        params: &str,
    ) -> Result<String> {
        if method.is_empty() {
            return Err(Error::InvalidArgument("invoke_web_view_custom_method takes a method name"));
        }
        let request = functions::bots::InvokeWebViewCustomMethod {
            bot: self.resolve_user(&bot_id.into()).await?,
            custom_method: method.into(),
            params: base::DataJSON::DataJSON(Box::new(types::DataJSON { data: params.into() })),
        };
        let base::DataJSON::DataJSON(result) = self.invoke(&request).await?;
        Ok(result.data)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{base, types};

    #[tokio::test]
    async fn an_empty_method_is_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        let peer = types::InputPeerUser { user_id: 42, access_hash: 7 };
        client.0.peers.lock().unwrap().insert(42, base::InputPeer::InputPeerUser(Box::new(peer)));
        assert!(matches!(client.invoke_web_view_custom_method(42, "", "{}").await, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }
}
