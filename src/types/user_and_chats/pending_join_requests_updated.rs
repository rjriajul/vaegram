use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::{Chat, User};

/// The number of pending requests to join a chat this account administers changed.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct PendingJoinRequestsUpdated {
    /// The chat the requests are for.
    pub chat: Chat,
    /// How many join requests are waiting for approval now.
    pub pending_count: i32,
    /// Some of the users who asked to join most recently.
    pub recent_requesters: Vec<User>,
    client: Client,
}

impl PendingJoinRequestsUpdated {
    pub(crate) fn parse(client: &Client, raw: &types::UpdatePendingJoinRequests, entities: &Entities) -> Self {
        PendingJoinRequestsUpdated {
            chat: Chat::parse(&raw.peer, entities, false),
            pending_count: raw.requests_pending,
            recent_requesters: raw.recent_requesters.iter().map(|&id| User::parse(id, entities)).collect(),
            client: client.clone(),
        }
    }

    /// The [`Client`] that received this update.
    pub fn client(&self) -> &Client {
        &self.client
    }
}
