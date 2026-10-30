use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::AttachMenuBot;

impl Client {
    pub async fn get_attach_menu_bots(&self) -> Result<Vec<AttachMenuBot>> {
        let request = functions::messages::GetAttachMenuBots { hash: 0 };
        let base::AttachMenuBots::AttachMenuBots(found) = self.invoke(&request).await? else {
            return Err(Error::Unexpected("messages.getAttachMenuBots returned attachMenuBotsNotModified"));
        };
        let types::AttachMenuBots { bots, users, .. } = *found;
        let entities = Entities::new(self, users, Vec::new());
        Ok(bots.into_iter().map(|base::AttachMenuBot::AttachMenuBot(b)| AttachMenuBot::parse(*b, &entities)).collect())
    }
}
