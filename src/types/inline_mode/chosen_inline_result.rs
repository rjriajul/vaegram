use std::fmt;

use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::{InlineMessageId, Location, User};

/// An inline result the user picked; arrives only with inline feedback enabled in @BotFather.
#[derive(Clone)]
pub struct ChosenInlineResult {
    pub result_id: String,
    pub from_user: User,
    pub location: Option<Location>,
    /// Set only when the message has an inline keyboard, so it can be edited later.
    pub inline_message_id: Option<InlineMessageId>,
    pub query: String,
    client: Client,
}

impl ChosenInlineResult {
    pub(crate) fn parse(client: &Client, raw: &types::UpdateBotInlineSend, entities: &Entities) -> ChosenInlineResult {
        ChosenInlineResult {
            result_id: raw.id.clone(),
            from_user: User::parse(raw.user_id, entities),
            location: raw.geo.as_ref().and_then(Location::parse),
            inline_message_id: raw.msg_id.clone().map(InlineMessageId),
            query: raw.query.clone(),
            client: client.clone(),
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }
}

impl fmt::Debug for ChosenInlineResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChosenInlineResult")
            .field("result_id", &self.result_id)
            .field("from_user", &self.from_user)
            .field("location", &self.location)
            .field("inline_message_id", &self.inline_message_id)
            .field("query", &self.query)
            .finish_non_exhaustive()
    }
}
