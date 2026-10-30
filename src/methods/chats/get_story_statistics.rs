use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{MessageStatistics, PeerRef};

impl Client {
    pub async fn get_story_statistics(&self, chat_id: impl Into<PeerRef>, story_id: i32) -> Result<MessageStatistics> {
        let peer = self.resolve_peer(chat_id).await?;
        let base::stats::StoryStats::StoryStats(s) =
            self.invoke(&functions::stats::GetStoryStats { dark: false, peer, id: story_id }).await?;
        Ok(MessageStatistics::parse(s.views_graph, s.reactions_by_emotion_graph))
    }
}
