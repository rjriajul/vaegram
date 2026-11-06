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
///
/// Set `limit` before awaiting; without it this reads the whole history.
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

    /// Starts at this offset token, taken from [`next_offset`](Self::next_offset) of an earlier pager, instead of
    /// the first transaction.
    pub fn offset(mut self, offset: impl Into<String>) -> Self {
        self.pages.set_offset(offset.into());
        self
    }

    /// Where the unread part of the history starts, for `offset` on a later pager; `None` once the history has
    /// ended. Transactions already fetched but not yet returned by `next` come before it.
    pub fn next_offset(&self) -> Option<&str> {
        self.pages.offset().map(String::as_str)
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

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};
    use crate::utils::get_peer;

    fn transaction(id: &str) -> base::StarsTransaction {
        let mut buf = Vec::new();
        0u32.serialize(&mut buf);
        id.to_owned().serialize(&mut buf);
        base::StarsAmount::StarsAmount(Box::new(types::StarsAmount { amount: 50, nanos: 0 })).serialize(&mut buf);
        1i32.serialize(&mut buf);
        let peer = types::StarsTransactionPeer { peer: get_peer(7) };
        base::StarsTransactionPeer::StarsTransactionPeer(Box::new(peer)).serialize(&mut buf);
        let transaction = types::StarsTransaction::deserialize_bare(&mut &buf[..]).unwrap();
        base::StarsTransaction::StarsTransaction(Box::new(transaction))
    }

    fn status(ids: &[&str], next_offset: Option<&str>) -> crate::Result<Vec<u8>> {
        let status = types::payments::StarsStatus {
            balance: base::StarsAmount::StarsAmount(Box::new(types::StarsAmount { amount: 0, nanos: 0 })),
            subscriptions: None,
            subscriptions_next_offset: None,
            subscriptions_missing_balance: None,
            history: Some(ids.iter().map(|id| transaction(id)).collect()),
            next_offset: next_offset.map(Into::into),
            chats: Vec::new(),
            users: Vec::new(),
        };
        Ok(base::payments::StarsStatus::StarsStatus(Box::new(status)).to_bytes())
    }

    fn request(offset: &str, limit: i32) -> Vec<u8> {
        let request = functions::payments::GetStarsTransactions {
            inbound: false,
            outbound: false,
            ascending: false,
            ton: false,
            subscription_id: None,
            peer: base::InputPeer::InputPeerSelf,
            offset: offset.into(),
            limit,
        };
        request.to_bytes()
    }

    #[tokio::test]
    async fn reads_the_history_across_pages() {
        let (client, mut sent) = Client::scripted(vec![status(&["a", "b"], Some("p2")), status(&["c"], None)]);
        let history = client.get_star_transactions().await.unwrap();
        assert_eq!(history.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
        assert_eq!(sent.try_recv().unwrap(), request("", 100));
        assert_eq!(sent.try_recv().unwrap(), request("p2", 100));
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_later_pager_resumes_from_next_offset() {
        let (client, mut sent) = Client::scripted(vec![status(&["a", "b"], Some("p2")), status(&["c"], None)]);
        let mut first = client.get_star_transactions().limit(2);
        while first.next().await.unwrap().is_some() {}
        let resume = first.next_offset().map(str::to_owned).unwrap();
        let rest = client.get_star_transactions().offset(resume).await.unwrap();
        assert_eq!(rest.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["c"]);
        assert_eq!(sent.try_recv().unwrap(), request("", 2));
        assert_eq!(sent.try_recv().unwrap(), request("p2", 100));
        assert!(sent.try_recv().is_err());
    }
}
