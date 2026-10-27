use crate::Client;
use crate::errors::Result;
use crate::methods::advanced::ChatPeer;
use crate::raw::functions;
use crate::types::ChatPrivileges;

impl Client {
    /// In basic groups any privilege makes the user a full admin, and `ChatPrivileges::default()` demotes.
    pub async fn promote_chat_member(&self, chat_id: i64, user_id: i64, privileges: ChatPrivileges) -> Result<()> {
        match self.resolve_chat(chat_id).await? {
            ChatPeer::Group(chat_id) => {
                let request = functions::messages::EditChatAdmin {
                    chat_id,
                    user_id: self.resolve_user(user_id).await?,
                    is_admin: privileges != ChatPrivileges::default(),
                };
                self.invoke(&request).await?;
            }
            ChatPeer::Channel(channel) => {
                let request = functions::channels::EditAdmin {
                    channel,
                    user_id: self.resolve_user(user_id).await?,
                    admin_rights: privileges.write(),
                    rank: None,
                };
                self.invoke(&request).await?;
            }
        }
        Ok(())
    }
}
