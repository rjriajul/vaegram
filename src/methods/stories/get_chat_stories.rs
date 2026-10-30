use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{PeerRef, Story};

impl Client {
    pub async fn get_chat_stories(&self, chat_id: impl Into<PeerRef>) -> Result<Vec<Story>> {
        let peer = self.resolve_peer(chat_id).await?;
        let base::stories::PeerStories::PeerStories(s) =
            self.invoke(&functions::stories::GetPeerStories { peer }).await?;
        let base::PeerStories::PeerStories(stories) = s.stories;
        Ok(Story::parse_list(self, &stories.peer, stories.stories, s.users, s.chats))
    }
}
