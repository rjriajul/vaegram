use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{PeerRef, Story};
use crate::utils::get_peer;

impl Client {
    pub async fn get_stories(&self, chat_id: impl Into<PeerRef>, story_ids: &[i32]) -> Result<Vec<Story>> {
        if story_ids.is_empty() {
            return Err(Error::InvalidArgument("get_stories needs at least one story id"));
        }
        let (chat_id, peer) = self.resolve(&chat_id.into()).await?;
        let base::stories::Stories::Stories(s) =
            self.invoke(&functions::stories::GetStoriesByID { peer, id: story_ids.to_vec() }).await?;
        Ok(Story::parse_list(self, &get_peer(chat_id), s.stories, s.users, s.chats))
    }
}
