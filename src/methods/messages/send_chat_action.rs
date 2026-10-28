use crate::Client;
use crate::enums::ChatAction;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn send_chat_action(&self, chat_id: impl Into<PeerRef>, action: ChatAction) -> Result<()> {
        let peer = self.resolve_peer(chat_id).await?;
        self.invoke(&functions::messages::SetTyping { peer, top_msg_id: None, action: action.write() }).await?;
        Ok(())
    }
}
