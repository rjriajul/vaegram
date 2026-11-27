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
        // The Bot API renames a subscription link with a method of its own, `editChatSubscriptionInviteLink`.
        #[cfg(feature = "bot-api")]
        if let Some(bot) = self.0.bot_api.as_deref() {
            let chat_id = self.peer_id(chat_id.into()).await?;
            return bot.edit_chat_subscription_invite_link(self, chat_id, invite_link.into(), name.into()).await;
        }
        self.edit_chat_invite_link(chat_id, invite_link).name(name).await
    }
}
