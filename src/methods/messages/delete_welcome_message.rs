use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn delete_welcome_message(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> Result<bool> {
        let request =
            functions::ephemeral::DeleteWelcomeMessage { peer: self.resolve_peer(chat_id).await?, id: message_id };
        self.invoke(&request).await
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::functions::ephemeral::DeleteWelcomeMessage;
    use crate::raw::{Serializable, base, types};

    #[tokio::test]
    async fn returns_telegrams_answer() {
        let (client, mut rx) = Client::scripted(vec![Ok(true.to_bytes())]);
        assert!(client.delete_welcome_message(-5_i64, 7).await.unwrap());
        let peer = base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 5 });
        assert_eq!(rx.try_recv().unwrap(), DeleteWelcomeMessage { peer, id: 7 }.to_bytes());
    }

    #[tokio::test]
    async fn telegram_errors_come_back_unchanged() {
        let (client, _rx) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, "MESSAGE_ID_INVALID")))]);
        let result = client.delete_welcome_message(-5_i64, 7).await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "MESSAGE_ID_INVALID"), "{result:?}");
    }
}
