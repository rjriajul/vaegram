use std::fmt;

use crate::Client;
use crate::dispatcher::Entities;
use crate::methods::AnswerWidgetQuery;
use crate::raw::types;
use crate::types::{User, WidgetQueryType};

/// A query sent by a forwarded copy of a shared widget.
#[derive(Clone)]
pub struct SharedWidgetQuery {
    /// The query id, used to answer it.
    pub id: i64,
    /// The user who sent the query.
    pub from_user: User,
    /// What the widget requested.
    pub query_type: WidgetQueryType,
    /// Data from the widget. Invalid UTF-8 is replaced.
    pub data: String,
    /// Id of the shared widget.
    pub widget_id: i64,
    client: Client,
}

impl SharedWidgetQuery {
    pub(crate) fn parse(client: &Client, raw: &types::UpdateBotWidgetCopyQuery, entities: &Entities) -> Self {
        SharedWidgetQuery {
            id: raw.query_id,
            from_user: User::parse(raw.user_id, entities),
            query_type: WidgetQueryType::parse(&raw.query_type, raw.wallet_card.as_ref(), entities),
            data: String::from_utf8_lossy(&raw.data).into_owned(),
            widget_id: raw.widget_id,
            client: client.clone(),
        }
    }

    /// The client that received the update.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Starts an answer to this query. See [`Client::answer_widget_query`].
    pub fn answer(&self) -> AnswerWidgetQuery<'_> {
        self.client.answer_widget_query(self.id)
    }
}

impl fmt::Debug for SharedWidgetQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SharedWidgetQuery")
            .field("id", &self.id)
            .field("from_user", &self.from_user)
            .field("query_type", &self.query_type)
            .field("data", &self.data)
            .field("widget_id", &self.widget_id)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::SharedWidgetQuery;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};
    use crate::types::WidgetQueryType;

    #[test]
    fn parses_shared_widget_transaction_query() {
        let raw = types::UpdateBotWidgetCopyQuery {
            query_id: 42,
            query_type: base::WidgetQueryType::WidgetQueryTypeSentTransaction(Box::new(
                types::WidgetQueryTypeSentTransaction {
                    wallet: base::UserWallet::UserWallet(Box::new(types::UserWallet {
                        address: "wallet".into(),
                        balance: 50,
                        public_key: None,
                        state_init: None,
                    })),
                    msg_hash: "hash".into(),
                },
            )),
            user_id: 7,
            data: vec![b'x', 0xff],
            widget_id: 11,
            wallet_card: None,
        };
        let query = SharedWidgetQuery::parse(&Client::offline(), &raw, &Entities::default());
        assert_eq!((query.id, query.from_user.id, query.data.as_str(), query.widget_id), (42, 7, "x\u{fffd}", 11));
        let WidgetQueryType::SentTransaction { wallet, msg_hash } = query.query_type else {
            panic!("transaction query")
        };
        assert_eq!((wallet.address.as_str(), wallet.amount, msg_hash.as_str()), ("wallet", 50, "hash"));
        assert_eq!((wallet.public_key, wallet.state_init), (None, None));
    }
}
