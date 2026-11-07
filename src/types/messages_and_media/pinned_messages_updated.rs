use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::{base, types};
use crate::types::Chat;

/// Messages in a chat were pinned or unpinned.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct PinnedMessagesUpdated {
    /// The chat the messages are in.
    pub chat: Chat,
    /// Ids of the messages that were pinned or unpinned.
    pub message_ids: Vec<i32>,
    /// `true` if the messages were pinned, `false` if they were unpinned.
    pub is_pinned: bool,
    client: Client,
}

impl PinnedMessagesUpdated {
    pub(crate) fn parse(client: &Client, raw: types::UpdatePinnedMessages, entities: &Entities) -> Self {
        let chat = Chat::parse(&raw.peer, entities, false);
        PinnedMessagesUpdated { chat, message_ids: raw.messages, is_pinned: raw.pinned, client: client.clone() }
    }

    pub(crate) fn parse_channel(client: &Client, raw: types::UpdatePinnedChannelMessages, entities: &Entities) -> Self {
        let peer = base::Peer::PeerChannel(types::PeerChannel { channel_id: raw.channel_id });
        let chat = Chat::parse(&peer, entities, false);
        PinnedMessagesUpdated { chat, message_ids: raw.messages, is_pinned: raw.pinned, client: client.clone() }
    }

    /// The [`Client`] that received this update.
    pub fn client(&self) -> &Client {
        &self.client
    }
}
