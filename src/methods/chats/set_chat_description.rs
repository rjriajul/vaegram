use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn set_chat_description(&self, chat_id: i64, description: impl Into<String>) -> Result<()> {
        let peer = self.resolve_peer(chat_id).await?;
        self.invoke(&functions::messages::EditChatAbout { peer, about: description.into() }).await?;
        Ok(())
    }
}
