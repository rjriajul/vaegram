use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn delete_all_welcome_messages(&self, chat_id: impl Into<PeerRef>) -> Result<bool> {
        let request = functions::ephemeral::DeleteAllWelcomeMessages { peer: self.resolve_peer(chat_id).await? };
        self.invoke(&request).await
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::functions::ephemeral::DeleteAllWelcomeMessages;
    use crate::raw::{Serializable, base, types};

    #[tokio::test]
    async fn returns_telegrams_answer() {
        let (client, mut rx) = Client::scripted(vec![Ok(true.to_bytes())]);
        assert!(client.delete_all_welcome_messages(-5_i64).await.unwrap());
        let peer = base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 5 });
        assert_eq!(rx.try_recv().unwrap(), DeleteAllWelcomeMessages { peer }.to_bytes());
    }

    #[tokio::test]
    async fn telegram_errors_come_back_unchanged() {
        let (client, _rx) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, "PEER_ID_INVALID")))]);
        let result = client.delete_all_welcome_messages(-5_i64).await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "PEER_ID_INVALID"), "{result:?}");
    }
}
