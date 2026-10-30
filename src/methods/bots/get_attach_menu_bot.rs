use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::{AttachMenuBot, PeerRef};

impl Client {
    pub async fn get_attach_menu_bot(&self, bot_id: impl Into<PeerRef>) -> Result<AttachMenuBot> {
        let request = functions::messages::GetAttachMenuBot { bot: self.resolve_user(&bot_id.into()).await? };
        let base::AttachMenuBotsBot::AttachMenuBotsBot(found) = self.invoke(&request).await?;
        let types::AttachMenuBotsBot { bot: base::AttachMenuBot::AttachMenuBot(bot), users } = *found;
        Ok(AttachMenuBot::parse(*bot, &Entities::new(self, users, Vec::new())))
    }
}
