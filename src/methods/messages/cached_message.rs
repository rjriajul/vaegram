use crate::Client;
use crate::types::Message;

impl Client {
    /// The message `message_id` of the chat `chat_id` (a Bot-API-style id) from the message cache, without a
    /// request; `None` on a miss.
    ///
    /// Best effort: the cache is off unless
    /// [`max_message_cache_size`](crate::ClientBuilder::max_message_cache_size) is set, holds at most that many
    /// messages and drops the least recently used. Edits and deletes seen in updates refresh or drop a message, but
    /// one missed while offline, or a change Telegram sends no update for, leaves the copy stale. Use
    /// [`get_messages`](Client::get_messages) for the message as it is now.
    pub fn cached_message(&self, chat_id: i64, message_id: i32) -> Option<Message> {
        let cached = self.0.caches.message(chat_id, message_id)?;
        // Outside channels message ids are per account, so the cache can hold another chat's message under this id.
        Message::from_raw(self, cached.raw, &cached.entities).filter(|m| m.chat.id == chat_id)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::Client;
    use crate::dispatcher::cache::Caches;
    use crate::raw::base;
    use crate::types::Message;
    use crate::utils::{get_channel_id, get_peer};

    fn raw(id: i32, chat_id: i64) -> base::Message {
        base::Message::Message(Box::new(Message::bare(id, get_peer(chat_id), 1)))
    }

    #[tokio::test]
    async fn cached_message_reads_the_cache_without_a_request() {
        let (mut client, mut rx) = Client::recording();
        Arc::get_mut(&mut client.0).unwrap().caches = Caches::new(10, 0);
        let channel = get_channel_id(9);
        client.0.caches.store(&raw(4, 7), &Arc::default());
        client.0.caches.store(&raw(5, channel), &Arc::default());
        assert_eq!(client.cached_message(7, 4).map(|m| (m.id, m.chat.id)), Some((4, 7)));
        assert_eq!(client.cached_message(channel, 5).map(|m| (m.id, m.chat.id)), Some((5, channel)));
        // A miss, another chat's message under the same id, and another channel.
        assert!(client.cached_message(7, 6).is_none());
        assert!(client.cached_message(8, 4).is_none());
        assert!(client.cached_message(get_channel_id(10), 5).is_none());
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn cached_message_is_none_with_the_cache_off() {
        let (client, mut rx) = Client::recording();
        client.0.caches.store(&raw(4, 7), &Arc::default());
        assert!(client.cached_message(7, 4).is_none());
        assert!(rx.try_recv().is_err());
    }
}
