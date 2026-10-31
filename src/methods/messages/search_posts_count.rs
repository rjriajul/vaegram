use super::pages::count;
use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};

impl Client {
    /// Counts the public channel posts with this hashtag. Telegram allows a few free searches a day and refuses
    /// the rest with an error.
    pub async fn search_posts_count(&self, hashtag: &str) -> Result<i32> {
        let hashtag = hashtag.strip_prefix('#').unwrap_or(hashtag);
        if hashtag.is_empty() {
            return Err(Error::InvalidArgument("the hashtag is empty"));
        }
        let request = functions::channels::SearchPosts {
            hashtag: Some(hashtag.into()),
            query: None,
            offset_rate: 0,
            offset_peer: base::InputPeer::InputPeerEmpty,
            offset_id: 0,
            limit: 1,
            allow_paid_stars: None,
        };
        Ok(count(&self.invoke(&request).await?))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn counts_hashtag_posts() {
        let page = types::messages::ChannelMessages {
            inexact: false,
            pts: 1,
            count: 42,
            offset_id_offset: None,
            messages: Vec::new(),
            topics: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        let page = base::messages::Messages::ChannelMessages(Box::new(page));
        let (client, mut rx) = Client::scripted(vec![Ok(page.to_bytes())]);
        assert_eq!(client.search_posts_count("#rust").await.unwrap(), 42);
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
    }

    #[tokio::test]
    async fn an_empty_hashtag_is_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        for hashtag in ["", "#"] {
            let result = client.search_posts_count(hashtag).await;
            assert!(matches!(result, Err(Error::InvalidArgument(_))), "{hashtag}: {result:?}");
        }
        assert!(rx.try_recv().is_err());
    }
}
