use std::fmt;

use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::ChatType;
use crate::methods::AnswerInlineQuery;
use crate::raw::{base, types};
use crate::types::{InlineQueryResult, Location, User};

#[derive(Clone)]
pub struct InlineQuery {
    pub id: i64,
    pub from_user: User,
    pub query: String,
    pub offset: String,
    /// The kind of chat the query was sent from; `Private` also covers the chat with the bot itself.
    pub chat_type: Option<ChatType>,
    pub location: Option<Location>,
    client: Client,
}

impl InlineQuery {
    pub(crate) fn parse(client: &Client, raw: &types::UpdateBotInlineQuery, entities: &Entities) -> InlineQuery {
        use base::InlineQueryPeerType as T;
        let chat_type = raw.peer_type.as_ref().map(|t| match t {
            T::InlineQueryPeerTypeSameBotPM | T::InlineQueryPeerTypePM => ChatType::Private,
            T::InlineQueryPeerTypeBotPM => ChatType::Bot,
            T::InlineQueryPeerTypeChat => ChatType::Group,
            T::InlineQueryPeerTypeMegagroup => ChatType::Supergroup,
            T::InlineQueryPeerTypeBroadcast => ChatType::Channel,
        });
        InlineQuery {
            id: raw.query_id,
            from_user: User::parse(raw.user_id, entities),
            query: raw.query.clone(),
            offset: raw.offset.clone(),
            chat_type,
            location: raw.geo.as_ref().and_then(Location::parse),
            client: client.clone(),
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn answer<R: Into<InlineQueryResult>>(&self, results: impl IntoIterator<Item = R>) -> AnswerInlineQuery<'_> {
        self.client.answer_inline_query(self.id, results)
    }
}

impl fmt::Debug for InlineQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InlineQuery")
            .field("id", &self.id)
            .field("from_user", &self.from_user)
            .field("query", &self.query)
            .field("offset", &self.offset)
            .field("chat_type", &self.chat_type)
            .field("location", &self.location)
            .finish_non_exhaustive()
    }
}
