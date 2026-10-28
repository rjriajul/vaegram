use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{MenuButton, PeerRef};

impl Client {
    pub async fn get_chat_menu_button(&self, chat_id: Option<PeerRef>) -> Result<MenuButton> {
        let user_id = match chat_id {
            Some(id) => self.resolve_user(&id).await?,
            None => base::InputUser::InputUserEmpty,
        };
        let button = self.invoke(&functions::bots::GetBotMenuButton { user_id }).await?;
        Ok(MenuButton::parse(button))
    }
}
