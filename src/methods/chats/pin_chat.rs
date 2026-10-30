use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::PeerRef;

impl Client {
    pub async fn pin_chat(&self, chat_id: impl Into<PeerRef>) -> Result<()> {
        self.toggle_dialog_pin(chat_id, true).await
    }

    pub(crate) async fn toggle_dialog_pin(&self, chat_id: impl Into<PeerRef>, pinned: bool) -> Result<()> {
        let peer = types::InputDialogPeer { peer: self.resolve_peer(chat_id).await? };
        let peer = base::InputDialogPeer::InputDialogPeer(Box::new(peer));
        self.invoke(&functions::messages::ToggleDialogPin { pinned, peer }).await?;
        Ok(())
    }
}
