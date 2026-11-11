use std::fmt;

use crate::Client;
use crate::dispatcher::Entities;
use crate::methods::AnswerWidgetQuery;
use crate::raw::{base, types};
use crate::types::{Chat, InlineMessageId, TonWalletCard, User};

/// A query sent by a widget in a chat or an inline message.
#[derive(Clone)]
pub struct WidgetQuery {
    /// The query id, used to answer it.
    pub id: i64,
    /// The user who sent the query.
    pub from_user: User,
    /// The message containing the widget.
    pub origin: WidgetQueryOrigin,
    /// Identifies the chat the widget was shown in.
    pub chat_instance: i64,
    /// The chat type, as Telegram sends it.
    pub chat_type: String,
    /// What the widget requested.
    pub query_type: WidgetQueryType,
    /// Data from the widget. Invalid UTF-8 is replaced.
    pub data: String,
    /// Id of the shared widget, if any.
    pub widget_id: Option<i64>,
    client: Client,
}

/// Where a widget query originated.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum WidgetQueryOrigin {
    /// A regular message in a chat.
    #[non_exhaustive]
    Message {
        /// The chat the message is in.
        chat: Box<Chat>,
        /// Id of the message within the chat.
        message_id: i32,
    },
    /// A message sent through inline mode.
    Inline(InlineMessageId),
}

/// The action requested by a widget query.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WidgetQueryType {
    /// An ordinary data query.
    #[non_exhaustive]
    Data {
        /// The wallet card shared by the user, if any.
        wallet_card: Option<TonWalletCard>,
    },
    /// A request to sign JSON data with a wallet.
    #[non_exhaustive]
    SignData {
        /// The wallet used for signing.
        wallet: WidgetQueryWallet,
        /// The JSON string to sign.
        sign_data: String,
    },
    /// A transaction sent by the wallet.
    #[non_exhaustive]
    SentTransaction {
        /// The wallet that sent the transaction.
        wallet: WidgetQueryWallet,
        /// Hash of the sent message.
        msg_hash: String,
    },
}

/// A TON wallet shared with a widget query.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct WidgetQueryWallet {
    /// The wallet address.
    pub address: String,
    /// The wallet balance in nanotons.
    pub amount: i64,
    /// The wallet public key, if shared.
    pub public_key: Option<Vec<u8>>,
    /// The wallet's initial state, if shared.
    pub state_init: Option<Vec<u8>>,
}

impl WidgetQuery {
    pub(crate) fn parse(client: &Client, raw: &types::UpdateBotWidgetQuery, entities: &Entities) -> Self {
        WidgetQuery {
            id: raw.query_id,
            from_user: User::parse(raw.user_id, entities),
            origin: WidgetQueryOrigin::Message {
                chat: Box::new(Chat::parse(&raw.peer, entities, false)),
                message_id: raw.msg_id,
            },
            chat_instance: raw.chat_instance,
            chat_type: raw.chat_type.clone(),
            query_type: WidgetQueryType::parse(&raw.query_type, raw.wallet_card.as_ref(), entities),
            data: String::from_utf8_lossy(&raw.data).into_owned(),
            widget_id: raw.widget_id,
            client: client.clone(),
        }
    }

    pub(crate) fn parse_inline(client: &Client, raw: types::UpdateInlineBotWidgetQuery, entities: &Entities) -> Self {
        WidgetQuery {
            id: raw.query_id,
            from_user: User::parse(raw.user_id, entities),
            origin: WidgetQueryOrigin::Inline(InlineMessageId(raw.inline_message_id)),
            chat_instance: raw.chat_instance,
            chat_type: raw.chat_type,
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

    /// The chat of the message. `None` for inline messages.
    pub fn chat(&self) -> Option<&Chat> {
        match &self.origin {
            WidgetQueryOrigin::Message { chat, .. } => Some(chat),
            WidgetQueryOrigin::Inline(_) => None,
        }
    }

    /// Id of the message. `None` for inline messages.
    pub fn message_id(&self) -> Option<i32> {
        match &self.origin {
            WidgetQueryOrigin::Message { message_id, .. } => Some(*message_id),
            WidgetQueryOrigin::Inline(_) => None,
        }
    }

    /// The inline message id. `None` for regular messages.
    pub fn inline_message_id(&self) -> Option<&InlineMessageId> {
        if let WidgetQueryOrigin::Inline(id) = &self.origin { Some(id) } else { None }
    }
}

impl WidgetQueryType {
    pub(crate) fn parse(
        raw: &base::WidgetQueryType,
        wallet_card: Option<&base::wallet::Card>,
        entities: &Entities,
    ) -> Self {
        match raw {
            base::WidgetQueryType::WidgetQueryTypeData(_) => {
                WidgetQueryType::Data { wallet_card: wallet_card.map(|card| TonWalletCard::parse(card, entities)) }
            }
            base::WidgetQueryType::WidgetQueryTypeSignData(query) => {
                let base::DataJSON::DataJSON(json) = &query.sign_data;
                WidgetQueryType::SignData {
                    wallet: WidgetQueryWallet::parse(&query.wallet),
                    sign_data: json.data.clone(),
                }
            }
            base::WidgetQueryType::WidgetQueryTypeSentTransaction(query) => WidgetQueryType::SentTransaction {
                wallet: WidgetQueryWallet::parse(&query.wallet),
                msg_hash: query.msg_hash.clone(),
            },
        }
    }
}

impl WidgetQueryWallet {
    /// Creates a wallet with its `address` and its balance, `amount`, in nanotons, without a key.
    pub fn new(address: impl Into<String>, amount: i64) -> Self {
        WidgetQueryWallet { address: address.into(), amount, public_key: None, state_init: None }
    }

    /// Shares the wallet's public key and initial state. Telegram takes the two only together.
    #[must_use]
    pub fn keys(mut self, public_key: impl Into<Vec<u8>>, state_init: impl Into<Vec<u8>>) -> Self {
        self.public_key = Some(public_key.into());
        self.state_init = Some(state_init.into());
        self
    }

    fn parse(raw: &base::UserWallet) -> Self {
        let base::UserWallet::UserWallet(wallet) = raw;
        WidgetQueryWallet {
            address: wallet.address.clone(),
            amount: wallet.balance,
            public_key: wallet.public_key.clone(),
            state_init: wallet.state_init.clone(),
        }
    }
}

impl fmt::Debug for WidgetQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WidgetQuery")
            .field("id", &self.id)
            .field("from_user", &self.from_user)
            .field("origin", &self.origin)
            .field("chat_instance", &self.chat_instance)
            .field("chat_type", &self.chat_type)
            .field("query_type", &self.query_type)
            .field("data", &self.data)
            .field("widget_id", &self.widget_id)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::{WidgetQuery, WidgetQueryType};
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};
    use crate::types::user_and_chats::ton_wallet_card::tests::wallet_card;
    use crate::utils::get_peer;

    #[test]
    fn parses_chat_data_query_with_wallet_card() {
        let raw = types::UpdateBotWidgetQuery {
            query_id: 42,
            query_type: base::WidgetQueryType::WidgetQueryTypeData(types::WidgetQueryTypeData { with_wallet: true }),
            user_id: 7,
            peer: get_peer(8),
            msg_id: 9,
            chat_instance: 10,
            chat_type: "future_chat_type".into(),
            data: vec![b'x', 0xff],
            widget_id: Some(11),
            wallet_card: Some(wallet_card()),
        };
        let query = WidgetQuery::parse(&Client::offline(), &raw, &Entities::default());
        assert_eq!(
            (query.id, query.from_user.id, query.chat().map(|c| c.id), query.message_id()),
            (42, 7, Some(8), Some(9))
        );
        assert_eq!(
            (query.chat_instance, query.chat_type.as_str(), query.data.as_str(), query.widget_id),
            (10, "future_chat_type", "x\u{fffd}", Some(11))
        );
        assert!(query.inline_message_id().is_none());
        let WidgetQueryType::Data { wallet_card: Some(card) } = query.query_type else { panic!("data query") };
        assert_eq!((card.address.as_str(), card.amount, card.stickers.len()), ("wallet", 1234, 1));
    }

    #[test]
    fn parses_inline_sign_data_query() {
        let raw = types::UpdateInlineBotWidgetQuery {
            query_id: 12,
            query_type: base::WidgetQueryType::WidgetQueryTypeSignData(Box::new(types::WidgetQueryTypeSignData {
                wallet: base::UserWallet::UserWallet(Box::new(types::UserWallet {
                    address: "wallet".into(),
                    balance: 50,
                    public_key: Some(vec![1, 2]),
                    state_init: Some(vec![3, 4]),
                })),
                sign_data: base::DataJSON::DataJSON(Box::new(types::DataJSON { data: "{\"a\":1}".into() })),
            })),
            user_id: 7,
            inline_message_id: base::InputBotInlineMessageID::InputBotInlineMessageID64(Box::new(
                types::InputBotInlineMessageID64 { dc_id: 2, owner_id: 3, id: 4, access_hash: 5 },
            )),
            chat_instance: 13,
            chat_type: "private".into(),
            data: b"sign".to_vec(),
            widget_id: None,
            wallet_card: None,
        };
        let query = WidgetQuery::parse_inline(&Client::offline(), raw, &Entities::default());
        assert_eq!(
            (
                query.id,
                query.from_user.id,
                query.chat_instance,
                query.chat_type.as_str(),
                query.data.as_str(),
                query.widget_id
            ),
            (12, 7, 13, "private", "sign", None)
        );
        assert!(query.chat().is_none() && query.message_id().is_none());
        assert_eq!(query.inline_message_id().map(|id| id.dc_id()), Some(2));
        let WidgetQueryType::SignData { wallet, sign_data } = query.query_type else { panic!("sign data query") };
        assert_eq!((wallet.address.as_str(), wallet.amount, sign_data.as_str()), ("wallet", 50, "{\"a\":1}"));
        assert_eq!((wallet.public_key, wallet.state_init), (Some(vec![1, 2]), Some(vec![3, 4])));
    }
}
