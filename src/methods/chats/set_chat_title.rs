use crate::Client;
use crate::errors::Result;
use crate::methods::advanced::ChatPeer;
use crate::raw::functions;

impl Client {
    pub async fn set_chat_title(&self, chat_id: i64, title: impl Into<String>) -> Result<()> {
        let title = title.into();
        match self.resolve_chat(chat_id).await? {
            ChatPeer::Group(chat_id) => {
                self.invoke(&functions::messages::EditChatTitle { chat_id, title }).await?;
            }
            ChatPeer::Channel(channel) => {
                self.invoke(&functions::channels::EditTitle { channel, title }).await?;
            }
        }
        Ok(())
    }
}
