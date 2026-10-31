use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::PeerRef;

impl Client {
    /// Returns how many more stories the chat can post now; limits reached come back as Telegram errors.
    pub async fn can_post_stories(&self, chat_id: impl Into<PeerRef>) -> Result<i32> {
        let request = functions::stories::CanSendStory { peer: self.resolve_peer(chat_id).await? };
        let base::stories::CanSendStoryCount::CanSendStoryCount(count) = self.invoke(&request).await?;
        Ok(count.count_remains)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::functions::stories;
    use crate::raw::{Serializable, base, types};

    #[tokio::test]
    async fn returns_the_stories_left() {
        let reply =
            base::stories::CanSendStoryCount::CanSendStoryCount(types::stories::CanSendStoryCount { count_remains: 3 });
        let (client, mut rx) = Client::scripted(vec![Ok(reply.to_bytes())]);
        assert_eq!(client.can_post_stories("me").await.unwrap(), 3);
        let expected = stories::CanSendStory { peer: base::InputPeer::InputPeerSelf };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn story_limits_come_back_as_errors() {
        let (client, _rx) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, "STORIES_TOO_MUCH")))]);
        let result = client.can_post_stories("me").await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "STORIES_TOO_MUCH"), "{result:?}");
    }
}
