use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::PeerRef;

impl Client {
    pub async fn mark_chat_unread(&self, chat_id: impl Into<PeerRef>) -> Result<()> {
        let peer = types::InputDialogPeer { peer: self.resolve_peer(chat_id).await? };
        let peer = base::InputDialogPeer::InputDialogPeer(Box::new(peer));
        self.invoke(&functions::messages::MarkDialogUnread { unread: true, parent_peer: None, peer }).await?;
        Ok(())
    }
}
