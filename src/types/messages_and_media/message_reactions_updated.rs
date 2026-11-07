use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::{Chat, MessageReactions};

/// The reactions on a message changed, as a user account sees them: the full new set, not who changed what.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct MessageReactionsUpdated {
    /// The chat the message is in.
    pub chat: Chat,
    /// Id of the message whose reactions changed.
    pub message_id: i32,
    /// Id of the forum topic or thread the message is in. `None` outside of one.
    pub message_thread_id: Option<i32>,
    /// The reactions the message has now.
    pub reactions: MessageReactions,
    client: Client,
}

impl MessageReactionsUpdated {
    pub(crate) fn parse(
        client: &Client,
        raw: &types::UpdateMessageReactions,
        entities: &Entities,
    ) -> MessageReactionsUpdated {
        MessageReactionsUpdated {
            chat: Chat::parse(&raw.peer, entities, false),
            message_id: raw.msg_id,
            message_thread_id: raw.top_msg_id,
            reactions: MessageReactions::parse(&raw.reactions, entities),
            client: client.clone(),
        }
    }

    /// The [`Client`] that received this update.
    pub fn client(&self) -> &Client {
        &self.client
    }
}
