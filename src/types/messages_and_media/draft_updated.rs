use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::{Chat, Draft};

/// The draft in a chat was saved or cleared, on this or another of the account's devices.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct DraftUpdated {
    /// The chat the draft is in.
    pub chat: Chat,
    /// Forum topic the draft belongs to. `None` outside topics.
    pub topic_id: Option<i32>,
    /// The draft now. `None` once it was cleared.
    pub draft: Option<Draft>,
}

impl DraftUpdated {
    pub(crate) fn parse(_client: &Client, raw: &types::UpdateDraftMessage, entities: &Entities) -> Self {
        let chat = Chat::parse(&raw.peer, entities, false);
        let draft = Draft::parse(&raw.draft, chat.clone(), raw.top_msg_id, entities);
        DraftUpdated { chat, topic_id: raw.top_msg_id, draft }
    }
}
