use crate::Client;
use crate::errors::Result;
use crate::methods::pages::wire_limit;
use crate::raw::functions;
use crate::types::{Message, PeerRef};

impl Client {
    /// The newest messages with a live location in the chat, up to `limit`.
    pub async fn get_recent_locations(&self, chat_id: impl Into<PeerRef>, limit: usize) -> Result<Vec<Message>> {
        let peer = self.resolve_peer(chat_id).await?;
        let limit = wire_limit(limit);
        let found = self.invoke(&functions::messages::GetRecentLocations { peer, limit, hash: 0 }).await?;
        let (messages, entities) = Message::unpack(self, found);
        Ok(messages.into_iter().filter_map(|m| Message::from_raw(self, m, &entities)).collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::functions::messages::GetRecentLocations;
    use crate::raw::{Serializable, base, types};
    use crate::types::Message;
    use crate::utils::get_peer;

    #[tokio::test]
    async fn asks_for_the_limit_and_reads_the_messages() {
        let messages = [4, 3].map(|id| base::Message::Message(Box::new(Message::bare(id, get_peer(7), 1))));
        let found = types::messages::Messages {
            messages: messages.to_vec(),
            topics: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        let (client, mut rx) =
            Client::scripted(vec![Ok(base::messages::Messages::Messages(Box::new(found)).to_bytes())]);
        let user = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 1 }));
        client.0.peers.lock().unwrap().insert(7, user.clone());

        let found = client.get_recent_locations(7, 20).await.unwrap();
        assert_eq!(found.iter().map(|m| m.id).collect::<Vec<_>>(), [4, 3]);
        assert_eq!(rx.try_recv().unwrap(), GetRecentLocations { peer: user, limit: 20, hash: 0 }.to_bytes());
    }

    #[tokio::test]
    async fn a_limit_past_i32_max_saturates() {
        let (client, mut rx) = Client::recording();
        let user = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 1 }));
        client.0.peers.lock().unwrap().insert(7, user.clone());

        let _ = client.get_recent_locations(7, usize::MAX).await;
        assert_eq!(rx.try_recv().unwrap(), GetRecentLocations { peer: user, limit: i32::MAX, hash: 0 }.to_bytes());
    }
}
