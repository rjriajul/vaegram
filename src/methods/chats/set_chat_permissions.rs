use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{ChatPermissions, PeerRef};

impl Client {
    /// Sets the permissions every member has by default in a group or supergroup.
    ///
    /// Works in basic groups too; these defaults are the only member limits a basic group has.
    ///
    /// Every permission is set on its own, like the Bot API's `use_independent_chat_permissions`: allowing other
    /// messages, link previews or polls does not also allow text messages or media. Set those fields together if
    /// one should imply the other.
    pub async fn set_chat_permissions(&self, chat_id: impl Into<PeerRef>, permissions: ChatPermissions) -> Result<()> {
        let request = functions::messages::EditChatDefaultBannedRights {
            peer: self.resolve_peer(chat_id).await?,
            banned_rights: base::ChatBannedRights::ChatBannedRights(Box::new(permissions.write(0))),
        };
        self.invoke(&request).await?;
        Ok(())
    }
}
