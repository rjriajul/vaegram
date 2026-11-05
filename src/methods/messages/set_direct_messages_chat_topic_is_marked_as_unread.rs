use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::PeerRef;

impl Client {
    /// Marks a direct-messages topic as read or unread.
    ///
    /// `chat_id` is the direct messages chat. `topic_id` is the id of the user writing to it.
    pub async fn set_direct_messages_chat_topic_is_marked_as_unread(
        &self,
        chat_id: impl Into<PeerRef>,
        topic_id: i64,
        is_marked_as_unread: bool,
    ) -> Result<()> {
        let parent_peer = Some(self.resolve_peer(chat_id).await?);
        let peer = types::InputDialogPeer { peer: self.resolve_peer(topic_id).await? };
        let peer = base::InputDialogPeer::InputDialogPeer(Box::new(peer));
        self.invoke(&functions::messages::MarkDialogUnread { unread: is_marked_as_unread, parent_peer, peer }).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn topic_unread_sets_parent_peer() {
        let (client, mut rx) = Client::recording();
        let user = |id| base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: id, access_hash: 1 }));
        for id in [9, 5] {
            client.0.peers.lock().unwrap().insert(id, user(id));
        }
        let _ = client.set_direct_messages_chat_topic_is_marked_as_unread(9_i64, 5, true).await;
        let peer = base::InputDialogPeer::InputDialogPeer(Box::new(types::InputDialogPeer { peer: user(5) }));
        let expected = functions::messages::MarkDialogUnread { unread: true, parent_peer: Some(user(9)), peer };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }
}
