use crate::Client;
use crate::enums::MessagesFilter;
use crate::errors::Result;
use crate::types::{Message, PeerRef};

impl Client {
    /// The newest pinned message, or `None` when nothing is pinned.
    pub async fn get_pinned_message(&self, chat_id: impl Into<PeerRef>) -> Result<Option<Message>> {
        self.search_messages(chat_id).filter(MessagesFilter::Pinned).limit(1).next().await
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::MessagesFilter;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::Message;
    use crate::utils::get_peer;

    fn found(messages: Vec<types::Message>) -> Vec<u8> {
        let found = types::messages::Messages {
            messages: messages.into_iter().map(|m| base::Message::Message(Box::new(m))).collect(),
            topics: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        base::messages::Messages::Messages(Box::new(found)).to_bytes()
    }

    #[tokio::test]
    async fn pinned_is_one_pinned_search() {
        let pinned = found(vec![Message::bare(5, get_peer(7), 1)]);
        let (client, mut rx) = Client::scripted(vec![Ok(pinned), Ok(found(Vec::new()))]);
        let peer = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 2 }));
        client.0.peers.lock().unwrap().insert(7, peer.clone());

        assert_eq!(client.get_pinned_message(7).await.unwrap().map(|m| m.id), Some(5));
        let expected = functions::messages::Search {
            peer,
            q: String::new(),
            from_id: None,
            saved_peer_id: None,
            saved_reaction: None,
            top_msg_id: None,
            filter: MessagesFilter::Pinned.write(),
            min_date: 0,
            max_date: 0,
            offset_id: 0,
            add_offset: 0,
            limit: 1,
            max_id: 0,
            min_id: 0,
            hash: 0,
        };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
        // An empty page means nothing is pinned.
        assert_eq!(client.get_pinned_message(7).await.unwrap().map(|m| m.id), None);
    }
}
