use crate::Client;
use crate::enums::{ChatMemberStatus, ChatMembersFilter};
use crate::errors::Result;
use crate::types::{ChatMember, PeerRef};

/// Request built by [`Client::get_chat_administrators`].
#[must_use = "a request does nothing until it is awaited"]
pub struct GetChatAdministrators<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    return_bots: bool,
}

impl Client {
    /// Returns the owner and administrators of a chat.
    ///
    /// Bot administrators are left out unless [`return_bots`](GetChatAdministrators::return_bots) is set.
    pub fn get_chat_administrators(&self, chat_id: impl Into<PeerRef>) -> GetChatAdministrators<'_> {
        GetChatAdministrators { client: self, chat_id: chat_id.into(), return_bots: false }
    }
}

impl GetChatAdministrators<'_> {
    /// Keeps bot administrators in the result. The current account is always kept.
    pub fn return_bots(mut self) -> Self {
        self.return_bots = true;
        self
    }

    async fn send(self) -> Result<Vec<ChatMember>> {
        let (client, return_bots) = (self.client, self.return_bots);
        let mut admins = self.all().await?;
        let me = client.0.me.lock().unwrap().id;
        admins.retain(|m| {
            matches!(m.status, ChatMemberStatus::Owner | ChatMemberStatus::Administrator)
                && (return_bots || !m.user.is_bot || m.user.id == me)
        });
        Ok(admins)
    }

    /// Every owner and administrator, bots included.
    async fn all(self) -> Result<Vec<ChatMember>> {
        let client = self.client;
        // The Bot API can't list a basic group's members, but `getChatAdministrators` reads them in any chat.
        #[cfg(feature = "bot-api")]
        if let Some(bot) = client.0.bot_api.as_deref() {
            return bot.get_chat_administrators(client, client.peer_id(self.chat_id).await?).await;
        }
        client.get_chat_members(self.chat_id).filter(ChatMembersFilter::Administrators).await
    }
}

into_future!(GetChatAdministrators, Vec<ChatMember>);
