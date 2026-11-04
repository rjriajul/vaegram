use crate::Client;
use crate::errors::Result;
use crate::methods::pages::DialogPages;
use crate::raw::functions;
use crate::types::{DirectMessagesChatTopic, PeerRef, SavedDialog};

/// Pages through the topics of a direct messages chat.
pub struct GetDirectMessagesTopics<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    exclude_pinned: bool,
    pages: DialogPages<DirectMessagesChatTopic>,
}

impl Client {
    /// `chat_id` is the direct messages chat, not its channel.
    pub fn get_direct_messages_topics(&self, chat_id: impl Into<PeerRef>) -> GetDirectMessagesTopics<'_> {
        GetDirectMessagesTopics {
            client: self,
            chat_id: chat_id.into(),
            exclude_pinned: false,
            pages: DialogPages::new(),
        }
    }
}

impl GetDirectMessagesTopics<'_> {
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    /// Leaves the pinned topics out.
    pub fn exclude_pinned(mut self) -> Self {
        self.exclude_pinned = true;
        self
    }

    pub async fn next(&mut self) -> Result<Option<DirectMessagesChatTopic>> {
        loop {
            if let Some(topic) = self.pages.pop() {
                return Ok(Some(topic));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let request = functions::messages::GetSavedDialogs {
                exclude_pinned: self.exclude_pinned,
                parent_peer: Some(self.client.resolve_peer(self.chat_id.clone()).await?),
                offset_date: self.pages.offset_date,
                offset_id: self.pages.offset_id,
                offset_peer: self.pages.offset_peer.clone(),
                limit,
                hash: 0,
            };
            let (topics, count, next) = SavedDialog::unpack_topics(self.client, self.client.invoke(&request).await?);
            self.pages.refill(self.client, topics, count < limit as usize, next).await?;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn topics_send_parent_peer_and_exclude_pinned() {
        let (client, mut rx) = Client::recording();
        let peer = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 9, access_hash: 1 }));
        client.0.peers.lock().unwrap().insert(9, peer.clone());
        let _ = client.get_direct_messages_topics(9_i64).limit(4).exclude_pinned().next().await;
        let expected = functions::messages::GetSavedDialogs {
            exclude_pinned: true,
            parent_peer: Some(peer),
            offset_date: 0,
            offset_id: 0,
            offset_peer: base::InputPeer::InputPeerEmpty,
            limit: 4,
            hash: 0,
        };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }
}
