use crate::Client;
use crate::errors::Result;
use crate::types::{ChatInviteLink, PeerRef};

impl Client {
    /// Renames a subscription link, the only thing Telegram lets you change on one.
    pub async fn edit_chat_subscription_invite_link(
        &self,
        chat_id: impl Into<PeerRef>,
        invite_link: impl Into<String>,
        name: impl Into<String>,
    ) -> Result<ChatInviteLink> {
        self.edit_chat_invite_link(chat_id, invite_link).name(name).await
    }
}
