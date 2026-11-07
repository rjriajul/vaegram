use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::{Chat, Poll};

/// An update with the new state of a poll, such as fresh results or its closing.
///
/// Telegram leaves the poll itself out of updates once the account has seen it. Such an update reaches handlers only
/// when the message cache is on (`max_message_cache_size`) and kept the poll from its message or an earlier update.
#[derive(Debug, Clone)]
pub struct PollUpdated {
    /// The poll as it now stands.
    pub poll: Poll,
    /// The chat holding the poll, when Telegram names it.
    pub chat: Option<Chat>,
    /// Id of the message with the poll, when Telegram names it.
    pub message_id: Option<i32>,
    client: Client,
}

impl PollUpdated {
    pub(crate) fn parse(client: &Client, raw: &types::UpdateMessagePoll, entities: &Entities) -> Option<PollUpdated> {
        let chat = raw.peer.as_ref().map(|peer| Chat::parse(peer, entities, false));
        let poll = match &raw.poll {
            Some(poll) => Poll::parse(poll, &raw.results, entities),
            None => {
                let (poll, poll_entities) = client.0.caches.poll(raw.poll_id)?;
                Poll::parse(&poll, &raw.results, &poll_entities)
            }
        };
        Some(PollUpdated { poll, chat, message_id: raw.msg_id, client: client.clone() })
    }

    /// The [`Client`] that received this update.
    pub fn client(&self) -> &Client {
        &self.client
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::super::poll::tests::{raw_poll, results};
    use super::PollUpdated;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::dispatcher::cache::Caches;
    use crate::raw::{base, types};
    use crate::types::Message;
    use crate::utils::get_peer;

    #[test]
    fn results_without_the_poll_take_it_from_the_cache() {
        let mut client = Client::offline();
        Arc::get_mut(&mut client.0).unwrap().caches = Caches::new(10, 0);
        let media = types::MessageMediaPoll { poll: raw_poll(), results: results(&[], 0), attached_media: None };
        let media = Some(base::MessageMedia::MessageMediaPoll(Box::new(media)));
        let message = types::Message { media, ..Message::bare(5, get_peer(7), 0) };
        client.0.caches.store(&base::Message::Message(Box::new(message)), &Arc::default());
        let update = |poll_id: i64, chat: Option<i64>| types::UpdateMessagePoll {
            peer: chat.map(get_peer),
            msg_id: chat.map(|_| 5),
            top_msg_id: None,
            poll_id,
            poll: None,
            results: results(&[(1, 5, true)], 5),
        };
        let parsed = PollUpdated::parse(&client, &update(9, Some(7)), &Entities::default()).expect("the cached poll");
        let poll = &parsed.poll;
        assert_eq!((poll.question.as_str(), poll.total_voter_count, poll.correct_option_id), ("2 + 2?", 5, Some(1)));
        assert_eq!((parsed.chat.map(|c| c.id), parsed.message_id), (Some(7), Some(5)));
        // The poll id alone finds it; a poll never seen can't be reported.
        assert!(PollUpdated::parse(&client, &update(9, None), &Entities::default()).is_some());
        assert!(PollUpdated::parse(&client, &update(8, None), &Entities::default()).is_none());
    }
}
