use crate::Client;
use crate::errors::Result;
use crate::methods::advanced::ChatPeer;
use crate::raw::{base, functions};

impl Client {
    pub async fn leave_chat(&self, chat_id: i64) -> Result<()> {
        match self.resolve_chat(chat_id).await? {
            ChatPeer::Group(chat_id) => {
                let user_id = base::InputUser::InputUserSelf;
                self.invoke(&functions::messages::DeleteChatUser { revoke_history: false, chat_id, user_id }).await?;
            }
            ChatPeer::Channel(channel) => {
                self.invoke(&functions::channels::LeaveChannel { channel }).await?;
            }
        }
        Ok(())
    }
}
