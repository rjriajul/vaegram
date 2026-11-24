use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::{Chat, MessageReactions};

/// The reactions on a message changed, as a user account sees them: the full new set, not who changed what.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ReactionsUpdated {
    /// The chat the message is in.
    pub chat: Chat,
    /// Id of the message whose reactions changed.
    pub message_id: i32,
    /// Id of the forum topic or thread the message is in. `None` outside of one.
    pub message_thread_id: Option<i32>,
    /// The reactions the message has now.
    pub reactions: MessageReactions,
}

impl ReactionsUpdated {
    pub(crate) fn parse(
        _client: &Client,
        raw: &types::UpdateMessageReactions,
        entities: &Entities,
    ) -> ReactionsUpdated {
        ReactionsUpdated {
            chat: Chat::parse(&raw.peer, entities, false),
            message_id: raw.msg_id,
            message_thread_id: raw.top_msg_id,
            reactions: MessageReactions::parse(&raw.reactions, entities),
        }
    }
}
