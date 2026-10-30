use super::pages::Pages;
use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::Message;

#[must_use = "call .next() to fetch messages"]
pub struct SearchPosts<'a> {
    client: &'a Client,
    query: Option<String>,
    hashtag: Option<String>,
    allow_paid_stars: Option<i64>,
    pages: Pages,
}

impl Client {
    /// Telegram allows a few free searches a day; after that each one costs Stars.
    pub fn search_posts(&self) -> SearchPosts<'_> {
        SearchPosts { client: self, query: None, hashtag: None, allow_paid_stars: None, pages: Pages::by_rate() }
    }
}

impl SearchPosts<'_> {
    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = Some(query.into());
        self
    }

    pub fn hashtag(mut self, hashtag: impl Into<String>) -> Self {
        let mut hashtag = hashtag.into();
        // Telegram takes the hashtag without its `#`.
        if hashtag.starts_with('#') {
            hashtag.remove(0);
        }
        self.hashtag = Some(hashtag);
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    pub fn allow_paid_stars(mut self, stars: i64) -> Self {
        self.allow_paid_stars = Some(stars);
        self
    }

    pub async fn next(&mut self) -> Result<Option<Message>> {
        if self.query.is_some() == self.hashtag.is_some() {
            return Err(Error::InvalidArgument("search_posts takes either a query or a hashtag"));
        }
        loop {
            if let Some(message) = self.pages.pop(self.client) {
                return Ok(Some(message));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let request = functions::channels::SearchPosts {
                hashtag: self.hashtag.clone(),
                query: self.query.clone(),
                offset_rate: self.pages.offset_rate,
                offset_peer: self.pages.offset_peer(self.client).await?,
                offset_id: self.pages.offset_id,
                limit,
                allow_paid_stars: self.allow_paid_stars,
            };
            let page = self.client.invoke(&request).await?;
            self.pages.fill(self.client, page);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions};

    #[tokio::test]
    async fn exactly_one_of_query_and_hashtag() {
        let (client, mut rx) = Client::recording();
        assert!(matches!(client.search_posts().next().await, Err(Error::InvalidArgument(_))));
        let both = client.search_posts().query("rust").hashtag("rust").next().await;
        assert!(matches!(both, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());

        let _ = client.search_posts().hashtag("#rust").next().await;
        let expected = functions::channels::SearchPosts {
            hashtag: Some("rust".into()),
            query: None,
            offset_rate: 0,
            offset_peer: base::InputPeer::InputPeerEmpty,
            offset_id: 0,
            limit: 100,
            allow_paid_stars: None,
        };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }
}
