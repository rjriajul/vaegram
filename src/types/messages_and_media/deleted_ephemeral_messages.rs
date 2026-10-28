use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::Chat;

#[derive(Debug, Clone)]
pub struct DeletedEphemeralMessages {
    pub chat: Chat,
    pub ids: Vec<i32>,
    client: Client,
}

impl DeletedEphemeralMessages {
    pub(crate) fn parse(client: &Client, raw: types::UpdateDeleteEphemeralMessages, entities: &Entities) -> Self {
        DeletedEphemeralMessages { chat: Chat::parse(&raw.peer, entities, false), ids: raw.ids, client: client.clone() }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }
}
