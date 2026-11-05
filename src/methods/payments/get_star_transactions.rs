use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{PeerRef, StarTransaction, StarTransactions};

/// Request built by [`Client::get_star_transactions`].
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
    /// Lists Star transactions of the account, 100 to a page.
    ///
    /// Awaiting returns a page of [`StarTransactions`]. Pass its `next_offset` to [`GetStarTransactions::offset`].
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

    /// Lists only transactions that brought Stars in.
    pub fn inbound(mut self) -> Self {
        self.inbound = true;
        self
    }

    /// Lists only transactions that took Stars out.
    pub fn outbound(mut self) -> Self {
        self.outbound = true;
        self
    }

    /// Lists the oldest transactions first.
    pub fn ascending(mut self) -> Self {
        self.ascending = true;
        self
    }

    /// Continues from the `next_offset` of a previous page.
    pub fn offset(mut self, offset: impl Into<String>) -> Self {
        self.offset = offset.into();
        self
    }

    /// Caps how many transactions one page holds. The default is 100.
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
