use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::PeerRef;

#[must_use = "a request does nothing until it is awaited"]
/// Request built by [`Client::delete_direct_messages_chat_topic_history`]. Await it to delete.
pub struct DeleteDirectMessagesChatTopicHistory<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    topic_id: i64,
    max_id: i32,
    min_date: Option<i32>,
    max_date: Option<i32>,
}

impl Client {
    /// Deletes the history of one topic in a direct messages chat.
    ///
    /// `chat_id` is the direct messages chat and `topic_id` is the id of the user writing to it. Telegram deletes in
    /// batches, so awaiting repeats the request until nothing is left.
    pub fn delete_direct_messages_chat_topic_history(
        &self,
        chat_id: impl Into<PeerRef>,
        topic_id: i64,
    ) -> DeleteDirectMessagesChatTopicHistory<'_> {
        DeleteDirectMessagesChatTopicHistory {
            client: self,
            chat_id: chat_id.into(),
            topic_id,
            max_id: 0,
            min_date: None,
            max_date: None,
        }
    }
}

impl DeleteDirectMessagesChatTopicHistory<'_> {
    /// Deletes only messages with an id up to this one.
    pub fn max_id(mut self, message_id: i32) -> Self {
        self.max_id = message_id;
        self
    }

    /// Deletes only messages sent after this Unix time.
    pub fn min_date(mut self, date: i32) -> Self {
        self.min_date = Some(date);
        self
    }

    /// Deletes only messages sent before this Unix time.
    pub fn max_date(mut self, date: i32) -> Self {
        self.max_date = Some(date);
        self
    }

    async fn send(self) -> Result<()> {
        let parent_peer = self.client.resolve_peer(self.chat_id).await?;
        let request = functions::messages::DeleteSavedHistory {
            parent_peer: Some(parent_peer),
            peer: self.client.resolve_peer(self.topic_id).await?,
            max_id: self.max_id,
            min_date: self.min_date,
            max_date: self.max_date,
        };
        // The answer's pts is deliberately not fed to the update tracker: it isn't known whether a topic deletion
        // counts in the channel's pts or the account's, and applying it to the wrong counter would skip updates.
        // The tracker's own gap recovery resyncs if anything moved.
        loop {
            let base::messages::AffectedHistory::AffectedHistory(affected) = self.client.invoke(&request).await?;
            // Telegram works in batches and returns a non-zero offset until it is done.
            if affected.offset == 0 {
                return Ok(());
            }
        }
    }
}

into_future!(DeleteDirectMessagesChatTopicHistory, ());

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    fn user(id: i64) -> base::InputPeer {
        base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: id, access_hash: 1 }))
    }

    #[tokio::test]
    async fn delete_topic_history_sends_dates() {
        let (client, mut rx) = Client::recording();
        for id in [9, 5] {
            client.0.peers.lock().unwrap().insert(id, user(id));
        }
        let _ = client.delete_direct_messages_chat_topic_history(9_i64, 5).max_id(8).min_date(100).max_date(200).await;
        let expected = functions::messages::DeleteSavedHistory {
            parent_peer: Some(user(9)),
            peer: user(5),
            max_id: 8,
            min_date: Some(100),
            max_date: Some(200),
        };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }
}
