use std::fmt;

use crate::Client;
use crate::dispatcher::Entities;
use crate::methods::AnswerInlineQuery;
use crate::raw::types;
use crate::types::{InlineQueryResultArticle, User};

#[derive(Clone)]
pub struct InlineQuery {
    pub id: i64,
    pub from_user: User,
    pub query: String,
    pub offset: String,
    client: Client,
}

impl InlineQuery {
    pub(crate) fn parse(client: &Client, raw: &types::UpdateBotInlineQuery, entities: &Entities) -> InlineQuery {
        InlineQuery {
            id: raw.query_id,
            from_user: User::parse(raw.user_id, entities),
            query: raw.query.clone(),
            offset: raw.offset.clone(),
            client: client.clone(),
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn answer(&self, results: impl IntoIterator<Item = InlineQueryResultArticle>) -> AnswerInlineQuery<'_> {
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
            .finish_non_exhaustive()
    }
}
