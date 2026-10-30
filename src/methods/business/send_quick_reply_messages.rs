use crate::Client;
use crate::crypto::random;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// An empty `message_ids` sends the whole shortcut; Telegram then gets no random ids, so a retry may send it twice.
    pub async fn send_quick_reply_messages(
        &self,
        chat_id: impl Into<PeerRef>,
        shortcut_id: i32,
        message_ids: &[i32],
    ) -> Result<()> {
        let peer = self.resolve_peer(chat_id).await?;
        let random_id = message_ids.iter().map(|_| i64::from_le_bytes(random())).collect();
        let id = message_ids.to_vec();
        self.invoke(&functions::messages::SendQuickReplyMessages { peer, shortcut_id, id, random_id }).await.map(drop)
    }
}
