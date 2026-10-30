use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn read_mentions(&self, chat_id: impl Into<PeerRef>) -> Result<()> {
        let (chat_id, peer) = self.resolve(&chat_id.into()).await?;
        self.affected_history(chat_id, &functions::messages::ReadMentions { peer, top_msg_id: None }).await
    }
}
