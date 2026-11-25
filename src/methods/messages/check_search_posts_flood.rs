use super::search_posts::SearchPostsFlood;
use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};

impl Client {
    /// Asks Telegram about the free post searches left, without running a search.
    ///
    /// With a `query`, [`query_is_free`](SearchPostsFlood::query_is_free) says whether searching for that text
    /// would cost Stars. See [`search_posts`](Client::search_posts) for the search itself.
    pub async fn check_search_posts_flood(&self, query: Option<&str>) -> Result<SearchPostsFlood> {
        let request = functions::channels::CheckSearchPostsFlood { query: query.map(str::to_owned) };
        let base::SearchPostsFlood::SearchPostsFlood(flood) = self.invoke(&request).await?;
        Ok(SearchPostsFlood::from_raw(&flood))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn reads_the_flood_state_without_searching() {
        let flood = types::SearchPostsFlood {
            query_is_free: true,
            total_daily: 3,
            remains: 2,
            wait_till: None,
            stars_amount: 25,
        };
        let reply = base::SearchPostsFlood::SearchPostsFlood(Box::new(flood)).to_bytes();
        let (client, mut rx) = Client::scripted_raw(vec![Ok(reply)]);
        let flood = client.check_search_posts_flood(Some("rust")).await.unwrap();
        assert_eq!((flood.query_is_free, flood.total_daily, flood.remains), (true, 3, 2));
        assert_eq!((flood.wait_till, flood.stars_amount), (None, 25));
        let expected = functions::channels::CheckSearchPostsFlood { query: Some("rust".into()) };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
        assert!(rx.try_recv().is_err());
    }
}
