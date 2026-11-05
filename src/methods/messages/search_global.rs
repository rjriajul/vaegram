use super::pages::{Pages, count};
use crate::Client;
use crate::enums::MessagesFilter;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::Message;

#[must_use = "call .next() to fetch messages"]
pub struct SearchGlobal<'a> {
    client: &'a Client,
    query: String,
    filter: MessagesFilter,
    broadcasts_only: bool,
    groups_only: bool,
    users_only: bool,
    min_date: i32,
    max_date: i32,
    pages: Pages,
}

impl Client {
    pub fn search_global(&self, query: impl Into<String>) -> SearchGlobal<'_> {
        SearchGlobal {
            client: self,
            query: query.into(),
            filter: MessagesFilter::Empty,
            broadcasts_only: false,
            groups_only: false,
            users_only: false,
            min_date: 0,
            max_date: 0,
            pages: Pages::by_rate(),
        }
    }
}

impl SearchGlobal<'_> {
    pub fn filter(mut self, filter: MessagesFilter) -> Self {
        self.filter = filter;
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    pub fn broadcasts_only(mut self) -> Self {
        self.broadcasts_only = true;
        self
    }

    pub fn groups_only(mut self) -> Self {
        self.groups_only = true;
        self
    }

    pub fn users_only(mut self) -> Self {
        self.users_only = true;
        self
    }

    pub fn min_date(mut self, date: i32) -> Self {
        self.min_date = date;
        self
    }

    pub fn max_date(mut self, date: i32) -> Self {
        self.max_date = date;
        self
    }

    /// How many messages match, whatever `limit` says: the count Telegram gave with the last page, or, before any
    /// page, from one request for a single message. Either way it's asked for once.
    pub async fn total(&mut self) -> Result<i32> {
        if let Some(total) = self.pages.total {
            return Ok(total);
        }
        let request = self.request(1, false).await?;
        let total = count(&self.client.invoke(&request).await?);
        self.pages.total = Some(total);
        Ok(total)
    }

    pub async fn next(&mut self) -> Result<Option<Message>> {
        loop {
            if let Some(message) = self.pages.pop(self.client) {
                return Ok(Some(message));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let request = self.request(limit, true).await?;
            let page = self.client.invoke(&request).await?;
            self.pages.fill(self.client, page);
            self.pages.prefetch(self.client).await;
        }
    }

    /// `paged` continues from the last page; without it the request starts at the newest message.
    async fn request(&self, limit: i32, paged: bool) -> Result<functions::messages::SearchGlobal> {
        let (offset_rate, offset_id) = if paged { (self.pages.offset_rate, self.pages.offset_id) } else { (0, 0) };
        let offset_peer =
            if paged { self.pages.offset_peer(self.client).await? } else { base::InputPeer::InputPeerEmpty };
        Ok(functions::messages::SearchGlobal {
            broadcasts_only: self.broadcasts_only,
            groups_only: self.groups_only,
            users_only: self.users_only,
            folder_id: None,
            community: None,
            q: self.query.clone(),
            filter: self.filter.write(),
            min_date: self.min_date,
            max_date: self.max_date,
            offset_rate,
            offset_peer,
            offset_id,
            limit,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::pages::tests::slice;
    use crate::Client;

    #[tokio::test]
    async fn total_reuses_the_fetched_page() {
        let (client, mut rx) = Client::scripted(vec![Ok(slice(&[5, 4], 12))]);
        let mut found = client.search_global("x").limit(1);
        assert_eq!(found.next().await.unwrap().map(|m| m.id), Some(5));
        assert_eq!(found.total().await.unwrap(), 12);
        assert_eq!(found.total().await.unwrap(), 12);
        assert!(rx.try_recv().is_ok() && rx.try_recv().is_err(), "total asked again");
    }

    #[tokio::test]
    async fn total_before_next_makes_one_request() {
        let (client, mut rx) = Client::scripted(vec![Ok(slice(&[5], 12))]);
        let mut found = client.search_global("x");
        assert_eq!(found.total().await.unwrap(), 12);
        assert_eq!(found.total().await.unwrap(), 12);
        let body = rx.try_recv().expect("no request sent");
        // The request ends with `limit`.
        assert_eq!(body[body.len() - 4..], 1i32.to_le_bytes());
        assert!(rx.try_recv().is_err(), "total asked twice");
    }
}
