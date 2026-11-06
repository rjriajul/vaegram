use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{PeerRef, Story};
use crate::utils::get_peer;

impl Client {
    /// Returns stories of a chat by id.
    ///
    /// Deleted stories and ids Telegram doesn't know are left out, so the result can be shorter than `story_ids`;
    /// match the stories by [`Story::id`]. Compare [`get_messages`](Client::get_messages), which keeps a `None` slot
    /// for each missing message.
    ///
    /// Fails with `Error::InvalidArgument` if `story_ids` is empty.
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
