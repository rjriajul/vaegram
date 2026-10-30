use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{PeerRef, StarTransaction, StarTransactions};

#[must_use = "a request does nothing until it is awaited"]
pub struct GetStarTransactions<'a> {
    client: &'a Client,
    chat_id: Option<PeerRef>,
    inbound: bool,
    outbound: bool,
    ascending: bool,
    offset: String,
    limit: i32,
}

impl Client {
    pub fn get_star_transactions(&self) -> GetStarTransactions<'_> {
        GetStarTransactions {
            client: self,
            chat_id: None,
            inbound: false,
            outbound: false,
            ascending: false,
            offset: String::new(),
            limit: 100,
        }
    }
}

impl GetStarTransactions<'_> {
    /// A channel or bot the account owns instead of the account itself.
    pub fn chat(mut self, chat_id: impl Into<PeerRef>) -> Self {
        self.chat_id = Some(chat_id.into());
        self
    }

    pub fn inbound(mut self) -> Self {
        self.inbound = true;
        self
    }

    pub fn outbound(mut self) -> Self {
        self.outbound = true;
        self
    }

    pub fn ascending(mut self) -> Self {
        self.ascending = true;
        self
    }

    pub fn offset(mut self, offset: impl Into<String>) -> Self {
        self.offset = offset.into();
        self
    }

    pub fn limit(mut self, limit: i32) -> Self {
        self.limit = limit;
        self
    }

    async fn send(self) -> Result<StarTransactions> {
        let peer = match &self.chat_id {
            Some(chat) => self.client.resolve(chat).await?.1,
            None => base::InputPeer::InputPeerSelf,
        };
        let request = functions::payments::GetStarsTransactions {
            inbound: self.inbound,
            outbound: self.outbound,
            ascending: self.ascending,
            ton: false,
            subscription_id: None,
            peer,
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
