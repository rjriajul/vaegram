use crate::errors::Result;
use crate::types::{ForumTopic, Message, Story};

/// The id of the General topic of a forum.
const GENERAL_TOPIC_ID: i32 = 1;

impl Message {
    /// The story this message replies to. `None` when it replies to no story, or the story is gone.
    pub async fn reply_to_story(&self) -> Result<Option<Story>> {
        let (Some(story_id), Some(chat_id)) = (self.reply_to_story_id, self.reply_to_story_user_id) else {
            return Ok(None);
        };
        Ok(self.client().get_stories(chat_id, &[story_id]).await?.into_iter().next())
    }

    /// The forum topic this message belongs to: its thread, or the General topic when it has none. `None` outside
    /// forums.
    pub async fn topic(&self) -> Result<Option<ForumTopic>> {
        if !self.chat.is_forum {
            return Ok(None);
        }
        let topic_id = self.message_thread_id.unwrap_or(GENERAL_TOPIC_ID);
        let caches = &self.client().0.caches;
        if let Some(topic) = caches.topic(self.chat.id, topic_id) {
            return Ok(Some(topic));
        }
        let topic = self.client().get_forum_topics_by_id(self.chat.id, &[topic_id]).await?.into_iter().next();
        if let Some(topic) = &topic {
            caches.store_topic(self.chat.id, topic);
        }
        Ok(topic)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::functions::{messages, stories};
    use crate::raw::{Serializable, base, types};
    use crate::types::Message;
    use crate::utils::{get_channel_id, get_peer};

    fn message(client: &Client, chat: i64) -> Message {
        let raw = Message::bare(1, get_peer(chat), 0);
        Message::parse(client, Box::new(raw), &crate::dispatcher::Entities::default())
    }

    #[tokio::test]
    async fn no_story_reply_sends_nothing() {
        let (client, mut rx) = Client::recording();
        let m = message(&client, 7);
        assert!(m.reply_to_story().await.unwrap().is_none());
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_story_reply_fetches_that_story() {
        let (client, mut rx) = Client::recording();
        let mut m = message(&client, 7);
        (m.reply_to_story_id, m.reply_to_story_user_id) = (Some(4), Some(0));
        let _ = m.reply_to_story().await;
        let fetch = stories::GetStoriesByID { peer: base::InputPeer::InputPeerSelf, id: vec![4] };
        assert_eq!(rx.try_recv().unwrap(), fetch.to_bytes());
    }

    #[tokio::test]
    async fn topic_is_the_thread_or_general_in_a_forum() {
        let (client, mut rx) = Client::recording();
        let peer = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        let input = base::InputPeer::InputPeerChannel(Box::new(peer));
        client.0.peers.lock().unwrap().insert(get_channel_id(9), input.clone());

        let mut m = message(&client, get_channel_id(9));
        assert!(m.topic().await.unwrap().is_none(), "not a forum");
        assert!(rx.try_recv().is_err());

        m.chat.is_forum = true;
        let _ = m.topic().await;
        let general = messages::GetForumTopicsByID { peer: input.clone(), topics: vec![1] };
        assert_eq!(rx.try_recv().unwrap(), general.to_bytes());

        m.message_thread_id = Some(6);
        let _ = m.topic().await;
        assert_eq!(rx.try_recv().unwrap(), messages::GetForumTopicsByID { peer: input, topics: vec![6] }.to_bytes());
    }
}
