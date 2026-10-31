use crate::Client;
use crate::crypto::random;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{PeerRef, ReplyParameters};

impl Client {
    /// Tells a private chat that a screenshot of it was taken.
    pub async fn send_screenshot_notification(&self, chat_id: impl Into<PeerRef>) -> Result<()> {
        let (chat_id, peer) = self.resolve(&chat_id.into()).await?;
        let reply_to = ReplyParameters::new(0).write(self, chat_id).await?;
        let request = functions::messages::SendScreenshotNotification {
            peer,
            reply_to: base::InputReplyTo::InputReplyToMessage(Box::new(reply_to)),
            random_id: i64::from_le_bytes(random()),
        };
        self.invoke(&request).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn notifies_a_screenshot() {
        let updates = types::Updates { updates: Vec::new(), users: Vec::new(), chats: Vec::new(), date: 1, seq: 0 };
        let (client, mut rx) = Client::scripted(vec![Ok(base::Updates::Updates(Box::new(updates)).to_bytes())]);
        let peer = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 2 }));
        client.0.peers.lock().unwrap().insert(7, peer.clone());

        client.send_screenshot_notification(7).await.unwrap();
        let reply_to = types::InputReplyToMessage {
            reply_to_msg_id: 0,
            top_msg_id: None,
            reply_to_peer_id: None,
            quote_text: None,
            quote_entities: None,
            quote_offset: None,
            monoforum_peer_id: None,
            todo_item_id: None,
            poll_option: None,
            quote_rich: None,
        };
        let reply_to = base::InputReplyTo::InputReplyToMessage(Box::new(reply_to));
        let expected = functions::messages::SendScreenshotNotification { peer, reply_to, random_id: 0 }.to_bytes();
        let body = rx.try_recv().unwrap();
        // The last 8 bytes are the random_id.
        assert_eq!(body[..body.len() - 8], expected[..expected.len() - 8]);
    }
}
