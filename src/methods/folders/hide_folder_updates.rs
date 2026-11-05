use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

use super::create_folder_invite_link::chatlist;

impl Client {
    /// Hides the new chats of a shared folder that [`Client::get_folder_updates`] lists.
    pub async fn hide_folder_updates(&self, folder_id: i32) -> Result<()> {
        self.invoke(&functions::chatlists::HideChatlistUpdates { chatlist: chatlist(folder_id) }).await?;
        Ok(())
    }
}
