use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

use super::check_folder_invite_link::slug;
use super::create_folder_invite_link::chatlist;

impl Client {
    pub async fn delete_folder_invite_link(&self, folder_id: i32, invite_link: &str) -> Result<()> {
        let slug = slug(invite_link)?.to_owned();
        self.invoke(&functions::chatlists::DeleteExportedInvite { chatlist: chatlist(folder_id), slug }).await?;
        Ok(())
    }
}
