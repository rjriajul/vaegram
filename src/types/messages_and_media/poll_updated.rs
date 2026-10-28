use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::{Chat, Poll};

#[derive(Debug, Clone)]
pub struct PollUpdated {
    pub poll: Poll,
    pub chat: Option<Chat>,
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

    pub fn client(&self) -> &Client {
        &self.client
    }
}
