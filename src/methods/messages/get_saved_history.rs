use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{Message, PeerRef};

use super::pages::Pages;

/// Messages of Saved Messages from one source chat. Awaiting it collects the messages into a `Vec`, up to
/// [`limit`](GetSavedHistory::limit); [`next`](GetSavedHistory::next) reads them one at a time.
#[must_use = "a pager does nothing until it is awaited or read with next()"]
pub struct GetSavedHistory<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    offset_date: i32,
    pages: Pages,
    // Set for a direct-messages topic: the chat holding the topics, with `chat_id` the topic.
    parent: Option<PeerRef>,
    min_id: i32,
    max_id: i32,
}

impl Client {
    /// Reads the messages of Saved Messages that came from a chat, newest first.
    pub fn get_saved_history(&self, chat_id: impl Into<PeerRef>) -> GetSavedHistory<'_> {
        GetSavedHistory {
            client: self,
            chat_id: chat_id.into(),
            offset_date: 0,
            pages: Pages::new(),
            parent: None,
            min_id: 0,
            max_id: 0,
        }
    }
}

impl<'a> GetSavedHistory<'a> {
    pub(crate) fn of_topic(client: &'a Client, chat_id: PeerRef, topic: PeerRef) -> Self {
        GetSavedHistory { parent: Some(chat_id), ..client.get_saved_history(topic) }
    }

    pub(crate) fn min_id(mut self, message_id: i32) -> Self {
        self.min_id = message_id;
        self
    }

    pub(crate) fn max_id(mut self, message_id: i32) -> Self {
        self.max_id = message_id;
        self
    }

    /// Stops after this many messages. Without it the walk runs to the last message.
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    /// Starts below this message id, so the walk skips it and anything newer.
    pub fn offset_id(mut self, message_id: i32) -> Self {
        self.pages.offset_id = message_id;
        self
    }

    /// Starts at messages older than this Unix time.
    pub fn offset_date(mut self, date: i32) -> Self {
        self.offset_date = date;
        self
    }

    /// Returns the next message, fetching a page when the buffered one is used up. `None` after the last.
    pub async fn next(&mut self) -> Result<Option<Message>> {
        loop {
            if let Some(message) = self.pages.pop(self.client) {
                return Ok(Some(message));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let parent_peer = match &self.parent {
                Some(parent) => Some(self.client.resolve(parent).await?.1),
                None => None,
            };
            let request = functions::messages::GetSavedHistory {
                parent_peer,
                peer: self.client.resolve(&self.chat_id).await?.1,
                offset_id: self.pages.offset_id,
                offset_date: self.offset_date,
                add_offset: 0,
                limit,
                max_id: self.max_id,
                min_id: self.min_id,
                hash: 0,
            };
            let page = self.client.invoke(&request).await?;
            self.pages.fill(self.client, page);
            self.pages.prefetch(self.client).await;
        }
    }
}

pager!(GetSavedHistory, Message);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    fn client() -> (Client, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        let (client, rx) = Client::recording();
        for id in [9, 5] {
            let peer = types::InputPeerUser { user_id: id, access_hash: 1 };
            client.0.peers.lock().unwrap().insert(id, base::InputPeer::InputPeerUser(Box::new(peer)));
        }
        (client, rx)
    }

    fn user(id: i64) -> base::InputPeer {
        base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: id, access_hash: 1 }))
    }

    #[tokio::test]
    async fn saved_messages_still_send_no_parent() {
        let (client, mut rx) = client();
        let _ = client.get_saved_history(9_i64).limit(7).next().await;
        let expected = functions::messages::GetSavedHistory {
            parent_peer: None,
            peer: user(9),
            offset_id: 0,
            offset_date: 0,
            add_offset: 0,
            limit: 7,
            max_id: 0,
            min_id: 0,
            hash: 0,
        };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }
}
