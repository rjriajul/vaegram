use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn delete_saved_history(&self, chat_id: impl Into<PeerRef>) -> Result<()> {
        let request = functions::messages::DeleteSavedHistory {
            parent_peer: None,
            peer: self.resolve_peer(chat_id).await?,
            max_id: 0,
            min_date: None,
            max_date: None,
        };
        // Saved Messages live in your own chat, so the account's pts counts them whatever the sublist peer is.
        let me = self.peer_id(PeerRef::Me).await?;
        self.affected_history(me, &request).await
    }
}
