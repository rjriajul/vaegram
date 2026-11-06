use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::methods::pages::OffsetPages;
use crate::raw::{base, functions};
use crate::types::{PeerRef, StarTransaction};

/// The account's Star transactions, newest first, built by [`Client::get_star_transactions`].
///
/// Awaiting it collects the transactions into a `Vec`, up to [`limit`](GetStarTransactions::limit);
/// [`next`](GetStarTransactions::next) reads them one at a time. Transactions are fetched 100 to a page.
#[must_use = "a pager does nothing until it is awaited or read with next()"]
pub struct GetStarTransactions<'a> {
    client: &'a Client,
    chat_id: Option<PeerRef>,
    peer: Option<base::InputPeer>,
    inbound: bool,
    outbound: bool,
    ascending: bool,
    pages: OffsetPages<StarTransaction>,
}

impl Client {
    /// Lists the account's Star transactions, newest first.
    pub fn get_star_transactions(&self) -> GetStarTransactions<'_> {
        GetStarTransactions {
            client: self,
            chat_id: None,
            peer: None,
            inbound: false,
            outbound: false,
            ascending: false,
            pages: OffsetPages::new(100),
        }
    }
}

impl GetStarTransactions<'_> {
    /// Lists the transactions of a channel or bot the account owns instead of its own.
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

    /// Starts at this offset token, which Telegram hands out with each page, instead of the first transaction.
    pub fn offset(mut self, offset: impl Into<String>) -> Self {
        self.pages.set_offset(offset.into());
        self
    }

    /// Stops after this many transactions in total. The default reads the whole history.
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    /// Returns the next transaction, or `None` once the history or the limit is exhausted.
    pub async fn next(&mut self) -> Result<Option<StarTransaction>> {
        loop {
            if let Some(transaction) = self.pages.pop() {
                return Ok(Some(transaction));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let peer = match (&self.peer, &self.chat_id) {
                (Some(peer), _) => peer.clone(),
                (None, Some(chat)) => self.client.resolve(chat).await?.1,
                (None, None) => base::InputPeer::InputPeerSelf,
            };
            self.peer = Some(peer.clone());
            let request = functions::payments::GetStarsTransactions {
                inbound: self.inbound,
                outbound: self.outbound,
                ascending: self.ascending,
                ton: false,
                subscription_id: None,
                peer,
                offset: self.pages.next_offset().unwrap_or_default().into(),
                limit,
            };
            let base::payments::StarsStatus::StarsStatus(status) = self.client.invoke(&request).await?;
            let status = *status;
            let entities = Entities::new(self.client, status.users, status.chats);
            self.pages.refill(status.history.unwrap_or_default(), status.next_offset, |history| {
                history
                    .into_iter()
                    .map(|base::StarsTransaction::StarsTransaction(t)| StarTransaction::parse(*t, &entities))
                    .collect()
            });
        }
    }
}

pager!(GetStarTransactions, StarTransaction);
