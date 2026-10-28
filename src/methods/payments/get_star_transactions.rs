use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{StarTransaction, StarTransactions};

#[must_use = "a request does nothing until it is awaited"]
pub struct GetStarTransactions<'a> {
    client: &'a Client,
    offset: String,
    limit: i32,
}

impl Client {
    pub fn get_star_transactions(&self) -> GetStarTransactions<'_> {
        GetStarTransactions { client: self, offset: String::new(), limit: 100 }
    }
}

impl GetStarTransactions<'_> {
    pub fn offset(mut self, offset: impl Into<String>) -> Self {
        self.offset = offset.into();
        self
    }

    pub fn limit(mut self, limit: i32) -> Self {
        self.limit = limit;
        self
    }

    async fn send(self) -> Result<StarTransactions> {
        let request = functions::payments::GetStarsTransactions {
            inbound: false,
            outbound: false,
            ascending: false,
            ton: false,
            subscription_id: None,
            peer: base::InputPeer::InputPeerSelf,
            offset: self.offset,
            limit: self.limit,
        };
        let base::payments::StarsStatus::StarsStatus(status) = self.client.invoke(&request).await?;
        let status = *status;
        let entities = Entities::new(self.client, status.users, status.chats);
        let transactions = status
            .history
            .into_iter()
            .flatten()
            .map(|base::StarsTransaction::StarsTransaction(t)| StarTransaction::parse(*t, &entities))
            .collect();
        Ok(StarTransactions { transactions, next_offset: status.next_offset })
    }
}

into_future!(GetStarTransactions, StarTransactions);
