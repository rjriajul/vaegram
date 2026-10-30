use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn toggle_connected_bot_paused(&self, chat_id: impl Into<PeerRef>, paused: bool) -> Result<bool> {
        let peer = self.resolve_peer(chat_id).await?;
        self.invoke(&functions::account::ToggleConnectedBotPaused { peer, paused }).await
    }
}
