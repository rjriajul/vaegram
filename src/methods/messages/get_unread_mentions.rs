use super::pages::Pages;
use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{Message, PeerRef};

#[must_use = "call .next() to fetch messages"]
pub struct GetUnreadMentions<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    top_msg_id: Option<i32>,
    pages: Pages,
}

impl Client {
    /// The unread messages that mention you in a chat, newest first.
    pub fn get_unread_mentions(&self, chat_id: impl Into<PeerRef>) -> GetUnreadMentions<'_> {
        GetUnreadMentions { client: self, chat_id: chat_id.into(), top_msg_id: None, pages: Pages::new() }
    }
}

impl GetUnreadMentions<'_> {
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    /// Only the mentions in this forum topic.
    pub fn message_thread_id(mut self, message_thread_id: i32) -> Self {
        self.top_msg_id = Some(message_thread_id);
        self
    }

    pub async fn next(&mut self) -> Result<Option<Message>> {
        loop {
            if let Some(message) = self.pages.pop(self.client) {
                return Ok(Some(message));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let (chat_id, peer) = self.client.resolve(&self.chat_id).await?;
            self.chat_id = PeerRef::Id(chat_id);
            let request = functions::messages::GetUnreadMentions {
                peer,
                top_msg_id: self.top_msg_id,
                offset_id: self.pages.offset_id,
                add_offset: 0,
                limit,
                max_id: 0,
                min_id: 0,
            };
            let page = self.client.invoke(&request).await?;
            self.pages.fill(self.client, page);
        }
    }
}

/// Shared by the tests of the unread mentions, reactions and poll votes walks.
#[cfg(test)]
pub(super) mod paging {
    use tokio::sync::mpsc::UnboundedReceiver;

    use crate::Client;
    use crate::raw::{Serializable, base, types};
    use crate::types::Message;
    use crate::utils::get_peer;

    pub(crate) fn peer() -> base::InputPeer {
        base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 2 }))
    }

    /// A client whose answers are these pages of message ids, out of more in the chat.
    pub(crate) fn client(pages: &[&[i32]]) -> (Client, UnboundedReceiver<Vec<u8>>) {
        let page = |ids: &[i32]| {
            let page = types::messages::MessagesSlice {
                inexact: false,
                count: 50,
                next_rate: None,
                offset_id_offset: None,
                search_flood: None,
                messages: ids
                    .iter()
                    .map(|&id| base::Message::Message(Box::new(Message::bare(id, get_peer(7), 1))))
                    .collect(),
                topics: Vec::new(),
                chats: Vec::new(),
                users: Vec::new(),
            };
            Ok(base::messages::Messages::MessagesSlice(Box::new(page)).to_bytes())
        };
        let (client, rx) = Client::scripted(pages.iter().copied().map(page).collect());
        client.0.peers.lock().unwrap().insert(7, peer());
        (client, rx)
    }

    pub(crate) fn sent(rx: &mut UnboundedReceiver<Vec<u8>>) -> Vec<Vec<u8>> {
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::paging::{client, peer, sent};
    use crate::raw::Serializable;
    use crate::raw::functions::messages::GetUnreadMentions;

    fn request(top_msg_id: Option<i32>, offset_id: i32, limit: i32) -> Vec<u8> {
        let peer = peer();
        GetUnreadMentions { peer, top_msg_id, offset_id, add_offset: 0, limit, max_id: 0, min_id: 0 }.to_bytes()
    }

    #[tokio::test]
    async fn pages_back_from_the_last_one() {
        let (client, mut rx) = client(&[&[9, 8], &[5], &[]]);
        let mut mentions = client.get_unread_mentions(7);
        let mut ids = Vec::new();
        while let Some(message) = mentions.next().await.unwrap() {
            ids.push(message.id);
        }
        assert_eq!(ids, [9, 8, 5]);
        assert_eq!(sent(&mut rx), [request(None, 0, 100), request(None, 8, 100), request(None, 5, 100)]);
    }

    #[tokio::test]
    async fn the_topic_is_sent() {
        let (client, mut rx) = client(&[&[9]]);
        let mut mentions = client.get_unread_mentions(7).message_thread_id(3).limit(1);
        assert_eq!(mentions.next().await.unwrap().map(|m| m.id), Some(9));
        assert!(mentions.next().await.unwrap().is_none());
        assert_eq!(sent(&mut rx), [request(Some(3), 0, 1)]);
    }

    #[tokio::test]
    async fn limit_stops_paging() {
        let (client, mut rx) = client(&[&[9, 8], &[5, 4]]);
        let mut mentions = client.get_unread_mentions(7).limit(3);
        let mut ids = Vec::new();
        while let Some(message) = mentions.next().await.unwrap() {
            ids.push(message.id);
        }
        assert_eq!(ids, [9, 8, 5]);
        assert_eq!(sent(&mut rx), [request(None, 0, 3), request(None, 8, 1)]);
    }
}
