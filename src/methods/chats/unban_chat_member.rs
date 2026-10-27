use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::ChatPermissions;

impl Client {
    /// Lifts a ban or restriction in a supergroup or channel; the user has to join again by themselves.
    pub async fn unban_chat_member(&self, chat_id: i64, user_id: i64) -> Result<()> {
        let request = functions::channels::EditBanned {
            channel: self.resolve_channel(chat_id).await?,
            participant: self.resolve_peer(user_id).await?,
            banned_rights: base::ChatBannedRights::ChatBannedRights(Box::new(ChatPermissions::ALL.write(0))),
        };
        self.invoke(&request).await?;
        Ok(())
    }
}
