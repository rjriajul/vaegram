use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{ChatPermissions, PeerRef};

impl Client {
    /// Sets the permissions every member has by default in a group or supergroup.
    pub async fn set_chat_permissions(&self, chat_id: impl Into<PeerRef>, permissions: ChatPermissions) -> Result<()> {
        let request = functions::messages::EditChatDefaultBannedRights {
            peer: self.resolve_peer(chat_id).await?,
            banned_rights: base::ChatBannedRights::ChatBannedRights(Box::new(permissions.write(0))),
        };
        self.invoke(&request).await?;
        Ok(())
    }
}
