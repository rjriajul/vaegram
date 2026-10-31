use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{ForumTopic, PeerRef};

impl Client {
    /// Deleted topics are left out.
    pub async fn get_forum_topics_by_id(
        &self,
        chat_id: impl Into<PeerRef>,
        topic_ids: &[i32],
    ) -> Result<Vec<ForumTopic>> {
        if topic_ids.is_empty() {
            return Err(Error::InvalidArgument("get_forum_topics_by_id needs at least one topic id"));
        }
        let request = functions::messages::GetForumTopicsByID {
            peer: self.resolve_peer(chat_id).await?,
            topics: topic_ids.to_vec(),
        };
        let base::messages::ForumTopics::ForumTopics(page) = self.invoke(&request).await?;
        let page = *page;
        Entities::new(self, page.users, page.chats);
        Ok(page.topics.into_iter().filter_map(ForumTopic::parse).collect())
    }
}
