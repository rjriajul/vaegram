use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::Chat;

/// Messages deleted in a chat handled through a business connection.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct DeletedBusinessMessages {
    /// Id of the business connection the messages belong to.
    pub business_connection_id: String,
    /// The chat the messages were deleted from.
    pub chat: Chat,
    /// Ids of the deleted messages.
    pub message_ids: Vec<i32>,
}

impl DeletedBusinessMessages {
    pub(crate) fn parse(_client: &Client, raw: types::UpdateBotDeleteBusinessMessage, entities: &Entities) -> Self {
        DeletedBusinessMessages {
            chat: Chat::parse(&raw.peer, entities, false),
            business_connection_id: raw.connection_id,
            message_ids: raw.messages,
        }
    }
}
