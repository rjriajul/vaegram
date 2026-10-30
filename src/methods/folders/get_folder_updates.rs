use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::Chat;

use super::create_folder_invite_link::chatlist;

impl Client {
    pub async fn get_folder_updates(&self, folder_id: i32) -> Result<Vec<Chat>> {
        let request = functions::chatlists::GetChatlistUpdates { chatlist: chatlist(folder_id) };
        let base::chatlists::ChatlistUpdates::ChatlistUpdates(u) = self.invoke(&request).await?;
        let entities = Entities::new(self, u.users, u.chats);
        Ok(u.missing_peers.iter().map(|p| Chat::parse(p, &entities, false)).collect())
    }
}
