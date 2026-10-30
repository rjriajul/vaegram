use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn show_chat_stories(&self, chat_id: impl Into<PeerRef>) -> Result<()> {
        let peer = self.resolve_peer(chat_id).await?;
        self.invoke(&functions::stories::TogglePeerStoriesHidden { peer, hidden: false }).await?;
        Ok(())
    }
}
