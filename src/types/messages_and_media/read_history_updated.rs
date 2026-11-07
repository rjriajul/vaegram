use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::{base, types};
use crate::types::Chat;

/// Messages in a chat were read up to an id: incoming ones by this account (on any of its devices), or outgoing
/// ones by the other side.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ReadHistoryUpdated {
    /// The chat the messages are in.
    pub chat: Chat,
    /// Id of the newest message read; every message up to it counts as read.
    pub max_id: i32,
    /// `true` if the messages read are ones this account sent, `false` if this account read incoming ones.
    pub is_outgoing: bool,
    /// Id of the forum topic or comment thread the messages are in. `None` for the whole chat.
    pub message_thread_id: Option<i32>,
    /// How many incoming messages are still unread. `None` for outgoing messages and comment threads.
    pub unread_count: Option<i32>,
    client: Client,
}

impl ReadHistoryUpdated {
    fn new(client: &Client, chat: Chat, max_id: i32, is_outgoing: bool) -> Self {
        let client = client.clone();
        ReadHistoryUpdated { chat, max_id, is_outgoing, message_thread_id: None, unread_count: None, client }
    }

    fn channel(channel_id: i64, entities: &Entities) -> Chat {
        Chat::parse(&base::Peer::PeerChannel(types::PeerChannel { channel_id }), entities, false)
    }

    pub(crate) fn parse_inbox(client: &Client, raw: &types::UpdateReadHistoryInbox, entities: &Entities) -> Self {
        let read = Self::new(client, Chat::parse(&raw.peer, entities, false), raw.max_id, false);
        ReadHistoryUpdated { message_thread_id: raw.top_msg_id, unread_count: Some(raw.still_unread_count), ..read }
    }

    pub(crate) fn parse_outbox(client: &Client, raw: &types::UpdateReadHistoryOutbox, entities: &Entities) -> Self {
        Self::new(client, Chat::parse(&raw.peer, entities, false), raw.max_id, true)
    }

    pub(crate) fn parse_channel_inbox(
        client: &Client,
        raw: &types::UpdateReadChannelInbox,
        entities: &Entities,
    ) -> Self {
        let read = Self::new(client, Self::channel(raw.channel_id, entities), raw.max_id, false);
        ReadHistoryUpdated { unread_count: Some(raw.still_unread_count), ..read }
    }

    pub(crate) fn parse_channel_outbox(
        client: &Client,
        raw: &types::UpdateReadChannelOutbox,
        entities: &Entities,
    ) -> Self {
        Self::new(client, Self::channel(raw.channel_id, entities), raw.max_id, true)
    }

    pub(crate) fn parse_thread_inbox(
        client: &Client,
        raw: &types::UpdateReadChannelDiscussionInbox,
        entities: &Entities,
    ) -> Self {
        let read = Self::new(client, Self::channel(raw.channel_id, entities), raw.read_max_id, false);
        ReadHistoryUpdated { message_thread_id: Some(raw.top_msg_id), ..read }
    }

    pub(crate) fn parse_thread_outbox(
        client: &Client,
        raw: &types::UpdateReadChannelDiscussionOutbox,
        entities: &Entities,
    ) -> Self {
        let read = Self::new(client, Self::channel(raw.channel_id, entities), raw.read_max_id, true);
        ReadHistoryUpdated { message_thread_id: Some(raw.top_msg_id), ..read }
    }

    /// The [`Client`] that received this update.
    pub fn client(&self) -> &Client {
        &self.client
    }
}
