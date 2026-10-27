use crate::Client;
use crate::enums::{ChatMemberStatus, ChatMembersFilter};
use crate::errors::Result;
use crate::types::ChatMember;

pub struct GetChatAdministrators<'a> {
    client: &'a Client,
    chat_id: i64,
    return_bots: bool,
}

impl Client {
    pub fn get_chat_administrators(&self, chat_id: i64) -> GetChatAdministrators<'_> {
        GetChatAdministrators { client: self, chat_id, return_bots: false }
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
