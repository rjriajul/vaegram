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
        let mut admins = self.client.get_chat_members(self.chat_id).filter(ChatMembersFilter::Administrators).await?;
        let me = self.client.0.me.lock().unwrap().id;
        admins.retain(|m| {
            matches!(m.status, ChatMemberStatus::Owner | ChatMemberStatus::Administrator)
                && (self.return_bots || !m.user.is_bot || m.user.id == me)
        });
        Ok(admins)
    }
}

into_future!(GetChatAdministrators, Vec<ChatMember>);
