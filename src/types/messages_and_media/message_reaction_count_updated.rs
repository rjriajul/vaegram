use std::fmt;

use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::{Chat, ReactionCount};

/// An update to the reaction counts of a message.
#[derive(Clone)]
pub struct MessageReactionCountUpdated {
    /// The chat the message is in.
    pub chat: Chat,
    /// Id of the message that was reacted to.
    pub message_id: i32,
    /// Unix time of the change.
    pub date: i32,
    /// The reactions the message has now, with their counts.
    pub reactions: Vec<ReactionCount>,
    client: Client,
}

impl MessageReactionCountUpdated {
    pub(crate) fn parse(
        client: &Client,
        raw: &types::UpdateBotMessageReactions,
        entities: &Entities,
    ) -> MessageReactionCountUpdated {
        MessageReactionCountUpdated {
            chat: Chat::parse(&raw.peer, entities, false),
            message_id: raw.msg_id,
            date: raw.date,
            reactions: raw.reactions.iter().filter_map(ReactionCount::parse).collect(),
            client: client.clone(),
        }
    }

    /// The [`Client`] that received this update.
    pub fn client(&self) -> &Client {
        &self.client
    }
}

impl fmt::Debug for MessageReactionCountUpdated {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MessageReactionCountUpdated")
            .field("chat", &self.chat)
            .field("message_id", &self.message_id)
            .field("date", &self.date)
            .field("reactions", &self.reactions)
            .finish_non_exhaustive()
    }
}
