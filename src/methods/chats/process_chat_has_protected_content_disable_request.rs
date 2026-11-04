use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{Message, PeerRef};

impl Client {
    /// Answers the request in `request_message_id` to turn off a chat's protected content: `approve` is sent as the
    /// protection's new state. Returns the service message Telegram posts, when it posts one.
    pub async fn process_chat_has_protected_content_disable_request(
        &self,
        chat_id: impl Into<PeerRef>,
        request_message_id: i32,
        approve: bool,
    ) -> Result<Option<Message>> {
        let peer = self.resolve_peer(chat_id).await?;
        let request =
            functions::messages::ToggleNoForwards { peer, enabled: approve, request_msg_id: Some(request_message_id) };
        let updates = self.invoke(&request).await?;
        Ok(Message::all_from_updates(self, updates).next())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn protected_request_sends_request_msg_id() {
        let (client, mut rx) = Client::recording();
        let peer = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 9, access_hash: 1 }));
        client.0.peers.lock().unwrap().insert(9, peer.clone());
        let _ = client.process_chat_has_protected_content_disable_request(9_i64, 42, true).await;
        let expected = functions::messages::ToggleNoForwards { peer, enabled: true, request_msg_id: Some(42) };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }
}
