use crate::errors::Result;
use crate::types::{ForumTopic, Message, ServiceAction, Story};

/// The id of the General topic of a forum.
const GENERAL_TOPIC_ID: i32 = 1;

impl Message {
    /// The story this message replies to, or what `fetch_stories` fetched for it. `None` when it replies to no
    /// story, or the story is gone.
    pub async fn replied_story(&self) -> Result<Option<Story>> {
        let (Some(story_id), Some(chat_id)) = (self.reply_to_story_id, self.reply_to_story_user_id) else {
            return Ok(None);
        };
        if let Some(story) = self.prefetched.as_ref().and_then(|p| p.story.as_ref()) {
            return Ok(story.clone());
        }
        Ok(self.client().get_stories(chat_id, &[story_id]).await?.into_iter().next())
    }

    /// The forum topic this message belongs to: its thread, or the General topic when it has none. `None` outside
    /// forums.
    pub async fn topic(&self) -> Result<Option<ForumTopic>> {
        let Some(topic_id) = self.topic_id() else { return Ok(None) };
        if let Some(topic) = self.prefetched.as_ref().and_then(|p| p.topic.as_ref()) {
            return Ok(topic.clone());
        }
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

    /// The message a pin service message pinned: what `fetch_replies` fetched, the message cache's copy, or
    /// Telegram's answer. `None` for other messages, or when the pinned message is gone.
    pub async fn pinned_message(&self) -> Result<Option<Message>> {
        let Some(id) = self.pinned_id() else { return Ok(None) };
        if let Some(pinned) = self.prefetched.as_ref().and_then(|p| p.pinned.as_ref()) {
            return Ok(pinned.clone());
        }
        let client = self.client();
        let cached = client.0.caches.message(self.chat.id, id);
        if let Some(message) = cached.and_then(|c| Message::from_raw(client, c.raw, &c.entities)) {
            return Ok(Some(message));
        }
        Ok(client.get_messages(self.chat.id, &[id]).await?.pop().flatten())
    }

    pub(crate) fn topic_id(&self) -> Option<i32> {
        self.chat.is_forum.then(|| self.message_thread_id.unwrap_or(GENERAL_TOPIC_ID))
    }

    // A business chat's message ids belong to the other account, so the bot can't look them up.
    pub(crate) fn pinned_id(&self) -> Option<i32> {
        match self.service {
            Some(ServiceAction::PinnedMessage(id)) if self.business_connection_id.is_none() => Some(id),
            _ => None,
        }
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
        assert!(m.replied_story().await.unwrap().is_none());
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_story_reply_fetches_that_story() {
        let (client, mut rx) = Client::recording();
        let mut m = message(&client, 7);
        (m.reply_to_story_id, m.reply_to_story_user_id) = (Some(4), Some(0));
        let _ = m.replied_story().await;
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

        std::sync::Arc::make_mut(&mut m.chat).is_forum = true;
        let _ = m.topic().await;
        let general = messages::GetForumTopicsByID { peer: input.clone(), topics: vec![1] };
        assert_eq!(rx.try_recv().unwrap(), general.to_bytes());

        m.message_thread_id = Some(6);
        let _ = m.topic().await;
        assert_eq!(rx.try_recv().unwrap(), messages::GetForumTopicsByID { peer: input, topics: vec![6] }.to_bytes());
    }
}
