use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{Message, PeerRef};

impl Client {
    pub async fn get_scheduled_messages(&self, chat_id: impl Into<PeerRef>) -> Result<Vec<Message>> {
        let peer = self.resolve_peer(chat_id).await?;
        let found = self.invoke(&functions::messages::GetScheduledHistory { peer, hash: 0 }).await?;
        let (messages, entities) = Message::unpack(self, found);
        let scheduled = |mut m: Message| {
            m.scheduled = true;
            m
        };
        let mut found: Vec<_> =
            messages.into_iter().filter_map(|m| Message::from_raw(self, m, &entities)).map(scheduled).collect();
        self.prefetch(&mut found).await;
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, types};
    use crate::types::Message;
    use crate::utils::get_peer;

    #[tokio::test]
    async fn scheduled_messages_are_marked_scheduled() {
        let messages = types::messages::Messages {
            messages: vec![base::Message::Message(Box::new(Message::bare(3, get_peer(7), 1_900_000_000)))],
            topics: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        let (client, _rx) =
            Client::scripted(vec![Ok(base::messages::Messages::Messages(Box::new(messages)).to_bytes())]);
        let user = types::InputPeerUser { user_id: 7, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        let found = client.get_scheduled_messages(7).await.unwrap();
        assert_eq!(found.iter().map(|m| (m.id, m.scheduled)).collect::<Vec<_>>(), [(3, true)]);
    }
}
