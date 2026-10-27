use crate::Client;
use crate::enums::{ChatMemberStatus, ChatMembersFilter};
use crate::errors::Result;
use crate::types::{ChatMember, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct GetChatAdministrators<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    return_bots: bool,
}

impl Client {
    pub fn get_chat_administrators(&self, chat_id: impl Into<PeerRef>) -> GetChatAdministrators<'_> {
        GetChatAdministrators { client: self, chat_id: chat_id.into(), return_bots: false }
    }
}

impl GetChatAdministrators<'_> {
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
