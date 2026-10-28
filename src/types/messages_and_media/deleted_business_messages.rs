use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::Chat;

#[derive(Debug, Clone)]
pub struct DeletedBusinessMessages {
    pub business_connection_id: String,
    pub chat: Chat,
    pub message_ids: Vec<i32>,
    client: Client,
}

impl DeletedBusinessMessages {
    pub(crate) fn parse(client: &Client, raw: types::UpdateBotDeleteBusinessMessage, entities: &Entities) -> Self {
        DeletedBusinessMessages {
            chat: Chat::parse(&raw.peer, entities, false),
            business_connection_id: raw.connection_id,
            message_ids: raw.messages,
            client: client.clone(),
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }
}
