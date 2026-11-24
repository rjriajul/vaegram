use super::pages::{Pages, count};
use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{functions, types};
use crate::types::Message;

/// What Telegram says about the free post searches left, from [`SearchPosts::search_flood`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SearchPostsFlood {
    /// Whether this search was free.
    pub query_is_free: bool,
    /// How many free searches a day the account gets.
    pub total_daily: i32,
    /// How many free searches are left today.
    pub remains: i32,
    /// The Unix time the free searches reset, when none are left.
    pub wait_till: Option<i32>,
    /// The Stars a search costs once the free ones are used up.
    pub stars_amount: i64,
}

impl SearchPostsFlood {
    pub(super) fn from_raw(flood: &types::SearchPostsFlood) -> Self {
        SearchPostsFlood {
            query_is_free: flood.query_is_free,
            total_daily: flood.total_daily,
            remains: flood.remains,
            wait_till: flood.wait_till,
            stars_amount: flood.stars_amount,
        }
    }
}

/// Public channel posts found by [`Client::search_posts`]. Awaiting it collects the posts into a `Vec`, up to
/// [`limit`](SearchPosts::limit); [`next`](SearchPosts::next) reads them one at a time.
///
/// Set `limit` before awaiting; without it this reads every match (one request per 100 posts).
#[must_use = "a pager does nothing until it is awaited or read with next()"]
pub struct SearchPosts<'a> {
    client: &'a Client,
    query: Option<String>,
    hashtag: Option<String>,
    allow_paid_stars: Option<i64>,
    pages: Pages,
}

impl Client {
    /// Searches the posts of public channels by [`query`](SearchPosts::query) or [`hashtag`](SearchPosts::hashtag).
    ///
    /// Telegram allows a few free searches a day. After that each one costs Stars, up to the cap set with
    /// [`allow_paid_stars`](SearchPosts::allow_paid_stars).
    pub fn search_posts(&self) -> SearchPosts<'_> {
        SearchPosts { client: self, query: None, hashtag: None, allow_paid_stars: None, pages: Pages::by_rate() }
    }
}

impl SearchPosts<'_> {
    /// Searches for this text. Set either this or a hashtag, not both.
    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = Some(query.into());
        self
    }

    /// Searches for this hashtag, with or without its `#`. Set either this or a query, not both.
    pub fn hashtag(mut self, hashtag: impl Into<String>) -> Self {
        let mut hashtag = hashtag.into();
        // Telegram takes the hashtag without its `#`.
        if hashtag.starts_with('#') {
            hashtag.remove(0);
        }
        self.hashtag = Some(hashtag);
        self
    }

    /// Stops after this many posts. Without it the search runs to the oldest match.
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    /// The most Stars the search may cost once the free searches are used up.
    pub fn allow_paid_stars(mut self, stars: i64) -> Self {
        self.allow_paid_stars = Some(stars);
        self
    }

    /// Returns the next post, fetching a page when the buffered one is used up. `None` after the last post.
    ///
    /// Fails with `Error::InvalidArgument` unless exactly one of query and hashtag is set.
    pub async fn next(&mut self) -> Result<Option<Message>> {
        loop {
            if let Some(message) = self.pages.pop(self.client) {
                return Ok(Some(message));
            }
            if !self.fetch().await? {
                return Ok(None);
            }
        }
    }

    /// How many posts match, whatever `limit` says: the count Telegram gave with the last page, or, before any
    /// page, from one request for a single post, which can cost Stars like any search. Either way it's asked for once.
    pub async fn total(&mut self) -> Result<i32> {
        self.probe().await?;
        Ok(self.pages.total.unwrap_or(0))
    }

    /// The `next_rate` Telegram gave with the last page, which says where the next one starts; `None` if it gave
    /// none. Before any page it comes from one request for a single post, like [`total`](SearchPosts::total).
    pub async fn next_rate(&mut self) -> Result<Option<i32>> {
        self.probe().await?;
        Ok(self.pages.next_rate)
    }

    /// What Telegram said about the free searches left, or `None` if it said nothing. Before any page it comes from
    /// one request for a single post, like [`total`](SearchPosts::total).
    pub async fn search_flood(&mut self) -> Result<Option<SearchPostsFlood>> {
        self.probe().await?;
        Ok(self.pages.search_flood.clone())
    }

    /// Before any page was read, asks for a single post, ignoring `limit`, and keeps what the answer says.
    async fn probe(&mut self) -> Result<()> {
        if self.pages.total.is_some() {
            return Ok(());
        }
        let request = self.request(1).await?;
        let page = self.client.invoke(&request).await?;
        self.pages.total = Some(count(&page));
        self.pages.note(&page);
        Ok(())
    }

    /// Fetches the next page. `false` once the posts or the limit have run out.
    async fn fetch(&mut self) -> Result<bool> {
        self.check()?;
        let Some(limit) = self.pages.limit() else { return Ok(false) };
        let request = self.request(limit).await?;
        let page = self.client.invoke(&request).await?;
        self.pages.fill(self.client, page);
        self.pages.prefetch(self.client).await;
        Ok(true)
    }

    fn check(&self) -> Result<()> {
        if self.query.is_some() == self.hashtag.is_some() {
            return Err(Error::InvalidArgument("search_posts takes either a query or a hashtag"));
        }
        Ok(())
    }

    async fn request(&self, limit: i32) -> Result<functions::channels::SearchPosts> {
        self.check()?;
        Ok(functions::channels::SearchPosts {
            hashtag: self.hashtag.clone(),
            query: self.query.clone(),
            offset_rate: self.pages.offset_rate,
            offset_peer: self.pages.offset_peer(self.client).await?,
            offset_id: self.pages.offset_id,
            limit,
            allow_paid_stars: self.allow_paid_stars,
        })
    }
}

pager!(SearchPosts, Message);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn exactly_one_of_query_and_hashtag() {
        let (client, mut rx) = Client::recording_raw();
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

    #[tokio::test]
    async fn the_pager_exposes_count_next_rate_and_search_flood_whatever_the_limit() {
        let flood = types::SearchPostsFlood {
            query_is_free: false,
            total_daily: 3,
            remains: 0,
            wait_till: Some(900),
            stars_amount: 25,
        };
        let page = types::messages::MessagesSlice {
            inexact: false,
            count: 7,
            next_rate: Some(5),
            offset_id_offset: None,
            search_flood: Some(base::SearchPostsFlood::SearchPostsFlood(Box::new(flood))),
            messages: Vec::new(),
            topics: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        let reply = base::messages::Messages::MessagesSlice(Box::new(page)).to_bytes();
        let (client, mut rx) = Client::scripted_raw(vec![Ok(reply)]);
        let mut posts = client.search_posts().hashtag("rust").limit(0);
        assert_eq!(posts.total().await.unwrap(), 7);
        assert_eq!(posts.next_rate().await.unwrap(), Some(5));
        let flood = posts.search_flood().await.unwrap().unwrap();
        assert_eq!((flood.total_daily, flood.remains, flood.wait_till, flood.stars_amount), (3, 0, Some(900), 25));
        assert!(!flood.query_is_free);
        assert!(posts.next().await.unwrap().is_none());
        // One request for a single post served all of it.
        let expected = functions::channels::SearchPosts {
            hashtag: Some("rust".into()),
            query: None,
            offset_rate: 0,
            offset_peer: base::InputPeer::InputPeerEmpty,
            offset_id: 0,
            limit: 1,
            allow_paid_stars: None,
        };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
        assert!(rx.try_recv().is_err());
    }
}
