use crate::Client;
use crate::errors::Result;
use crate::types::{Message, PeerRef};

use super::get_saved_history::GetSavedHistory;

/// Pages through the messages of one topic of a direct messages chat, newest first.
pub struct GetDirectMessagesChatTopicHistory<'a>(GetSavedHistory<'a>);

impl Client {
    /// `chat_id` is the direct messages chat; `topic_id` is the id of the user writing to it.
    pub fn get_direct_messages_chat_topic_history(
        &self,
        chat_id: impl Into<PeerRef>,
        topic_id: i64,
    ) -> GetDirectMessagesChatTopicHistory<'_> {
        GetDirectMessagesChatTopicHistory(GetSavedHistory::of_topic(self, chat_id.into(), PeerRef::Id(topic_id)))
    }
}

impl GetDirectMessagesChatTopicHistory<'_> {
    pub fn limit(mut self, limit: usize) -> Self {
        self.0 = self.0.limit(limit);
        self
    }

    pub fn offset_id(mut self, message_id: i32) -> Self {
        self.0 = self.0.offset_id(message_id);
        self
    }

    pub fn offset_date(mut self, date: i32) -> Self {
        self.0 = self.0.offset_date(date);
        self
    }

    /// Stops before messages with an id up to this one.
    pub fn min_id(mut self, message_id: i32) -> Self {
        self.0 = self.0.min_id(message_id);
        self
    }

    /// Skips messages with an id from this one up.
    pub fn max_id(mut self, message_id: i32) -> Self {
        self.0 = self.0.max_id(message_id);
        self
    }

    pub async fn next(&mut self) -> Result<Option<Message>> {
        self.0.next().await
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    fn user(id: i64) -> base::InputPeer {
        base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: id, access_hash: 1 }))
    }

    #[tokio::test]
    async fn topic_history_sends_parent_and_topic() {
        let (client, mut rx) = Client::recording();
        for id in [9, 5] {
            client.0.peers.lock().unwrap().insert(id, user(id));
        }
        let _ = client.get_direct_messages_chat_topic_history(9_i64, 5).limit(3).min_id(2).max_id(40).next().await;
        let expected = functions::messages::GetSavedHistory {
            parent_peer: Some(user(9)),
            peer: user(5),
            offset_id: 0,
            offset_date: 0,
            add_offset: 0,
            limit: 3,
            max_id: 40,
            min_id: 2,
            hash: 0,
        };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }
}
