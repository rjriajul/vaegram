use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::{Chat, Poll};

/// An update with the new state of a poll, such as fresh results or its closing.
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
        Some(PollUpdated {
            poll: Poll::parse(raw.poll.as_ref()?, &raw.results, entities),
            chat: raw.peer.as_ref().map(|peer| Chat::parse(peer, entities, false)),
            message_id: raw.msg_id,
            client: client.clone(),
        })
    }

    /// The [`Client`] that received this update.
    pub fn client(&self) -> &Client {
        &self.client
    }
}
