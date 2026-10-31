use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{Message, PeerRef};
use crate::utils::get_peer_id;

impl Client {
    /// The whole rich message, for one Telegram sent only in part ([`RichMessage::part`]). `None` when the
    /// message is gone.
    ///
    /// [`RichMessage::part`]: crate::types::RichMessage::part
    pub async fn get_rich_message(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> Result<Option<Message>> {
        let (chat_id, peer) = self.resolve(&chat_id.into()).await?;
        let found = self.invoke(&functions::messages::GetRichMessage { peer, id: message_id }).await?;
        let (found, entities) = Message::unpack(self, found);
        Ok(found.into_iter().find_map(|m| match m {
            base::Message::Message(m) if m.id == message_id && get_peer_id(&m.peer_id) == chat_id => {
                Some(Message::parse(self, m, &entities))
            }
            _ => None,
        }))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::Message;
    use crate::utils::get_peer;

    fn user() -> base::InputPeer {
        base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 2 }))
    }

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
    async fn fetches_the_whole_rich_message() {
        let mut raw = Message::bare(5, get_peer(7), 1);
        let paragraph = base::RichText::TextPlain(Box::new(types::TextPlain { text: "hi".into() }));
        let blocks = vec![base::PageBlock::PageBlockParagraph(Box::new(types::PageBlockParagraph { text: paragraph }))];
        let rich = types::RichMessage { rtl: false, part: false, blocks, photos: vec![], documents: vec![] };
        raw.rich_message = Some(base::RichMessage::RichMessage(Box::new(rich)));
        let (client, mut rx) = Client::scripted(vec![Ok(found(vec![raw]))]);
        client.0.peers.lock().unwrap().insert(7, user());

        let message = client.get_rich_message(7, 5).await.unwrap().expect("the message was sent back");
        assert!(message.rich_message.is_some_and(|r| !r.part));
        let request = functions::messages::GetRichMessage { peer: user(), id: 5 };
        assert_eq!(rx.try_recv().unwrap(), request.to_bytes());
    }

    #[tokio::test]
    async fn another_chats_message_or_none_is_none() {
        let other_chat = Message::bare(5, get_peer(8), 1);
        let (client, _rx) = Client::scripted(vec![Ok(found(vec![other_chat])), Ok(found(Vec::new()))]);
        client.0.peers.lock().unwrap().insert(7, user());
        // Outside channels ids are per account, so message 5 of chat 8 isn't the one asked for.
        assert_eq!(client.get_rich_message(7, 5).await.unwrap().map(|m| m.id), None);
        // An empty reply means the message is gone.
        assert_eq!(client.get_rich_message(7, 5).await.unwrap().map(|m| m.id), None);
    }
}
