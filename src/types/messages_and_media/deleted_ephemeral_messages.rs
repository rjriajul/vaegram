use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::Chat;

/// Ephemeral messages that were deleted from a chat.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct DeletedEphemeralMessages {
    /// The chat the messages were deleted from.
    pub chat: Chat,
    /// Ids of the deleted messages.
    pub ids: Vec<i32>,
}

impl DeletedEphemeralMessages {
    pub(crate) fn parse(_client: &Client, raw: types::UpdateDeleteEphemeralMessages, entities: &Entities) -> Self {
        DeletedEphemeralMessages { chat: Chat::parse(&raw.peer, entities, false), ids: raw.ids }
    }
}
