use super::pages::Pages;
use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{Message, PeerRef};

/// Messages with unseen reactions. Awaiting it collects the messages into a `Vec`, up to
/// [`limit`](GetUnreadReactions::limit); [`next`](GetUnreadReactions::next) reads them one at a time.
#[must_use = "a pager does nothing until it is awaited or read with next()"]
pub struct GetUnreadReactions<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    top_msg_id: Option<i32>,
    pages: Pages,
}

impl Client {
    /// Walks your messages in a chat that have reactions you haven't seen, newest first.
    pub fn get_unread_reactions(&self, chat_id: impl Into<PeerRef>) -> GetUnreadReactions<'_> {
        GetUnreadReactions { client: self, chat_id: chat_id.into(), top_msg_id: None, pages: Pages::new() }
    }
}

impl GetUnreadReactions<'_> {
    /// Stops after this many messages. Without it the walk runs to the last message.
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    /// Only the messages in this forum topic.
    pub fn message_thread_id(mut self, message_thread_id: i32) -> Self {
        self.top_msg_id = Some(message_thread_id);
        self
    }

    /// Returns the next message, fetching a page when the buffered one is used up. `None` after the last.
    pub async fn next(&mut self) -> Result<Option<Message>> {
        loop {
            if let Some(message) = self.pages.pop(self.client) {
                return Ok(Some(message));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let (chat_id, peer) = self.client.resolve(&self.chat_id).await?;
            self.chat_id = PeerRef::Id(chat_id);
            let request = functions::messages::GetUnreadReactions {
                peer,
                top_msg_id: self.top_msg_id,
                saved_peer_id: None,
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

pager!(GetUnreadReactions, Message);

#[cfg(test)]
mod tests {
    use super::super::get_unread_mentions::paging::{client, peer, sent};
    use crate::raw::Serializable;
    use crate::raw::functions::messages::GetUnreadReactions;

    fn request(top_msg_id: Option<i32>, offset_id: i32, limit: i32) -> Vec<u8> {
        let peer = peer();
        let request = GetUnreadReactions {
            peer,
            top_msg_id,
            saved_peer_id: None,
            offset_id,
            add_offset: 0,
            limit,
            max_id: 0,
            min_id: 0,
        };
        request.to_bytes()
    }

    #[tokio::test]
    async fn pages_back_from_the_last_one() {
        let (client, mut rx) = client(&[&[9, 8], &[5], &[]]);
        let mut reactions = client.get_unread_reactions(7);
        let mut ids = Vec::new();
        while let Some(message) = reactions.next().await.unwrap() {
            ids.push(message.id);
        }
        assert_eq!(ids, [9, 8, 5]);
        assert_eq!(sent(&mut rx), [request(None, 0, 100), request(None, 8, 100), request(None, 5, 100)]);
    }

    #[tokio::test]
    async fn the_topic_is_sent() {
        let (client, mut rx) = client(&[&[9]]);
        let mut reactions = client.get_unread_reactions(7).message_thread_id(3).limit(1);
        assert_eq!(reactions.next().await.unwrap().map(|m| m.id), Some(9));
        assert!(reactions.next().await.unwrap().is_none());
        assert_eq!(sent(&mut rx), [request(Some(3), 0, 1)]);
    }

    #[tokio::test]
    async fn limit_stops_paging() {
        let (client, mut rx) = client(&[&[9, 8], &[5, 4]]);
        let mut reactions = client.get_unread_reactions(7).limit(3);
        let mut ids = Vec::new();
        while let Some(message) = reactions.next().await.unwrap() {
            ids.push(message.id);
        }
        assert_eq!(ids, [9, 8, 5]);
        assert_eq!(sent(&mut rx), [request(None, 0, 3), request(None, 8, 1)]);
    }
}
