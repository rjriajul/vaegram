use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::{base, types};
use crate::types::Chat;

/// `chat` is set only for channels and supergroups; Telegram sends bare ids for the rest.
#[derive(Debug, Clone)]
pub struct DeletedMessages {
    pub chat: Option<Chat>,
    pub message_ids: Vec<i32>,
    client: Client,
}

impl DeletedMessages {
    pub(crate) fn parse(client: &Client, raw: types::UpdateDeleteMessages) -> DeletedMessages {
        DeletedMessages { chat: None, message_ids: raw.messages, client: client.clone() }
    }

    pub(crate) fn parse_channel(
        client: &Client,
        raw: types::UpdateDeleteChannelMessages,
        entities: &Entities,
    ) -> DeletedMessages {
        let peer = base::Peer::PeerChannel(Box::new(types::PeerChannel { channel_id: raw.channel_id }));
        let chat = Some(Chat::parse(&peer, entities, false));
        DeletedMessages { chat, message_ids: raw.messages, client: client.clone() }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }
}
