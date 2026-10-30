use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::PeerRef;

impl Client {
    pub async fn export_story_link(&self, chat_id: impl Into<PeerRef>, story_id: i32) -> Result<String> {
        let peer = self.resolve_peer(chat_id).await?;
        let base::ExportedStoryLink::ExportedStoryLink(link) =
            self.invoke(&functions::stories::ExportStoryLink { peer, id: story_id }).await?;
        Ok(link.link)
    }
}
