use crate::raw::{base, functions, types};
use crate::types::{PeerRef, WidgetQueryWallet};
use crate::{
    Client,
    errors::{Error, Result},
};

/// Request built by [`Client::request_widget_query_answer`]. Awaiting it returns the bot's result.
#[must_use = "a request does nothing until it is awaited"]
pub struct RequestWidgetQueryAnswer<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    data: Vec<u8>,
    query_type: base::WidgetQueryType,
    // A wallet with only one of `public_key` and `state_init`: the two share a TL flag, so it can't be written.
    half_wallet: bool,
}

impl Client {
    /// Sends `data` (1-64 bytes) to the bot behind the widget in message `message_id`, as a tap in the widget does,
    /// and returns the bot's answer, which is `None` when it answered without data. Telegram allows each user 5
    /// queries per 5 seconds; the bot must answer within 15 seconds.
    pub fn request_widget_query_answer(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
        data: impl Into<Vec<u8>>,
    ) -> RequestWidgetQueryAnswer<'_> {
        RequestWidgetQueryAnswer {
            client: self,
            chat_id: chat_id.into(),
            message_id,
            data: data.into(),
            query_type: base::WidgetQueryType::WidgetQueryTypeData(types::WidgetQueryTypeData { with_wallet: false }),
            half_wallet: false,
        }
    }
}

fn raw_wallet(wallet: WidgetQueryWallet, half_wallet: &mut bool) -> base::UserWallet {
    *half_wallet = wallet.public_key.is_some() != wallet.state_init.is_some();
    base::UserWallet::UserWallet(Box::new(types::UserWallet {
        address: wallet.address,
        balance: wallet.amount,
        public_key: wallet.public_key,
        state_init: wallet.state_init,
    }))
}

impl RequestWidgetQueryAnswer<'_> {
    /// Attaches the user's TON wallet card to the query, which the bot gets as `wallet_card`.
    pub fn with_wallet(mut self) -> Self {
        self.query_type = base::WidgetQueryType::WidgetQueryTypeData(types::WidgetQueryTypeData { with_wallet: true });
        self.half_wallet = false;
        self
    }

    /// Makes this a query with `sign_data`, JSON data signed by `wallet` for the bot.
    ///
    /// Sending fails with [`Error::InvalidArgument`] if `wallet` has only one of `public_key` and `state_init`.
    pub fn sign_data(mut self, wallet: WidgetQueryWallet, sign_data: impl Into<String>) -> Self {
        self.query_type = base::WidgetQueryType::WidgetQueryTypeSignData(Box::new(types::WidgetQueryTypeSignData {
            wallet: raw_wallet(wallet, &mut self.half_wallet),
            sign_data: base::DataJSON::DataJSON(Box::new(types::DataJSON { data: sign_data.into() })),
        }));
        self
    }

    /// Makes this a query that tells the bot `wallet` sent a transaction with hash `msg_hash`.
    ///
    /// Sending fails with [`Error::InvalidArgument`] if `wallet` has only one of `public_key` and `state_init`.
    pub fn sent_transaction(mut self, wallet: WidgetQueryWallet, msg_hash: impl Into<String>) -> Self {
        self.query_type =
            base::WidgetQueryType::WidgetQueryTypeSentTransaction(Box::new(types::WidgetQueryTypeSentTransaction {
                wallet: raw_wallet(wallet, &mut self.half_wallet),
                msg_hash: msg_hash.into(),
            }));
        self
    }

    async fn send(self) -> Result<Option<String>> {
        if self.half_wallet {
            return Err(Error::InvalidArgument("a wallet needs both public_key and state_init, or neither"));
        }
        let request = functions::messages::GetBotWidgetQueryAnswer {
            peer: self.client.resolve_peer(self.chat_id).await?,
            msg_id: self.message_id,
            query_type: self.query_type,
            data: self.data,
        };
        let base::messages::BotWidgetQueryAnswer::BotWidgetQueryAnswer(answer) = self.client.invoke(&request).await?;
        Ok(answer.result)
    }
}
into_future!(RequestWidgetQueryAnswer, Option<String>);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn sends_widget_query_and_returns_bot_result() {
        let reply = base::messages::BotWidgetQueryAnswer::BotWidgetQueryAnswer(Box::new(
            types::messages::BotWidgetQueryAnswer { result: Some("answer".into()) },
        ));
        let (client, mut sent) = Client::scripted(vec![Ok(reply.to_bytes())]);
        let peer = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 42, access_hash: 7 }));
        {
            client.0.peers.lock().unwrap().insert(42, peer.clone());
        }
        let result = client.request_widget_query_answer(42, 12, b"query".to_vec()).with_wallet().await.unwrap();
        assert_eq!(result.as_deref(), Some("answer"));
        let expected = functions::messages::GetBotWidgetQueryAnswer {
            peer,
            msg_id: 12,
            query_type: base::WidgetQueryType::WidgetQueryTypeData(types::WidgetQueryTypeData { with_wallet: true }),
            data: b"query".to_vec(),
        };
        assert_eq!(sent.try_recv().unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn sends_wallet_query_kinds() {
        use crate::types::WidgetQueryWallet;

        let (client, mut sent) = Client::recording();
        let peer = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 42, access_hash: 7 }));
        {
            client.0.peers.lock().unwrap().insert(42, peer.clone());
        }
        let wallet = WidgetQueryWallet::new("wallet", 123).keys([1, 2], [3, 4]);
        let raw_wallet = || {
            base::UserWallet::UserWallet(Box::new(types::UserWallet {
                address: "wallet".into(),
                balance: 123,
                public_key: Some(vec![1, 2]),
                state_init: Some(vec![3, 4]),
            }))
        };
        let _ = client.request_widget_query_answer(42, 12, b"q".to_vec()).sign_data(wallet.clone(), "{}").await;
        let expected = functions::messages::GetBotWidgetQueryAnswer {
            peer: peer.clone(),
            msg_id: 12,
            data: b"q".to_vec(),
            query_type: base::WidgetQueryType::WidgetQueryTypeSignData(Box::new(types::WidgetQueryTypeSignData {
                wallet: raw_wallet(),
                sign_data: base::DataJSON::DataJSON(Box::new(types::DataJSON { data: "{}".into() })),
            })),
        };
        assert_eq!(sent.try_recv().unwrap(), expected.to_bytes());
        let _ = client.request_widget_query_answer(42, 12, b"q".to_vec()).sent_transaction(wallet, "hash").await;
        let expected = functions::messages::GetBotWidgetQueryAnswer {
            peer,
            msg_id: 12,
            data: b"q".to_vec(),
            query_type: base::WidgetQueryType::WidgetQueryTypeSentTransaction(Box::new(
                types::WidgetQueryTypeSentTransaction { wallet: raw_wallet(), msg_hash: "hash".into() },
            )),
        };
        assert_eq!(sent.try_recv().unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn a_wallet_with_one_key_field_is_refused_before_sending() {
        use crate::errors::Error;
        use crate::types::WidgetQueryWallet;

        let (client, mut sent) = Client::recording();
        let mut key_only = WidgetQueryWallet::new("w", 1);
        key_only.public_key = Some(vec![1]);
        let mut init_only = WidgetQueryWallet::new("w", 1);
        init_only.state_init = Some(vec![2]);
        let send = client.request_widget_query_answer(42, 12, b"q".to_vec()).sign_data(key_only, "{}");
        assert!(matches!(send.await, Err(Error::InvalidArgument(_))));
        let send = client.request_widget_query_answer(42, 12, b"q".to_vec()).sent_transaction(init_only, "hash");
        assert!(matches!(send.await, Err(Error::InvalidArgument(_))));
        assert!(sent.try_recv().is_err());
    }
}
