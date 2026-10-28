use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{ChatPermissions, PeerRef};

impl Client {
    /// Lifts a ban or restriction in a supergroup or channel; the user has to join again by themselves.
    pub async fn unban_chat_member(&self, chat_id: impl Into<PeerRef>, user_id: impl Into<PeerRef>) -> Result<()> {
        let request = functions::channels::EditBanned {
            channel: self.resolve_channel(&chat_id.into()).await?,
            participant: self.resolve_peer(user_id).await?,
            banned_rights: base::ChatBannedRights::ChatBannedRights(Box::new(ChatPermissions::ALL.write(0))),
        };
        self.invoke(&request).await?;
        Ok(())
    }
}
