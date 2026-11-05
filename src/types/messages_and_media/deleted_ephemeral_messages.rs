use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::Chat;

/// Ephemeral messages that were deleted from a chat.
#[derive(Debug, Clone)]
pub struct DeletedEphemeralMessages {
    /// The chat the messages were deleted from.
    pub chat: Chat,
    /// Ids of the deleted messages.
    pub ids: Vec<i32>,
    client: Client,
}

impl DeletedEphemeralMessages {
    pub(crate) fn parse(client: &Client, raw: types::UpdateDeleteEphemeralMessages, entities: &Entities) -> Self {
        DeletedEphemeralMessages { chat: Chat::parse(&raw.peer, entities, false), ids: raw.ids, client: client.clone() }
    }

    /// Returns the client that received the update.
    pub fn client(&self) -> &Client {
        &self.client
    }
}
