use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{MenuButton, PeerRef};

impl Client {
    pub async fn set_chat_menu_button(&self, chat_id: Option<PeerRef>, menu_button: MenuButton) -> Result<bool> {
        let user_id = match chat_id {
            Some(id) => self.resolve_user(&id).await?,
            None => base::InputUser::InputUserEmpty,
        };
        self.invoke(&functions::bots::SetBotMenuButton { user_id, button: menu_button.write() }).await
    }
}
