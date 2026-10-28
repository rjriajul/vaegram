use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::ChatPrivileges;

impl Client {
    pub async fn get_bot_default_privileges(&self, for_channels: bool) -> Result<Option<ChatPrivileges>> {
        let request = functions::users::GetFullUser { id: base::InputUser::InputUserSelf };
        let base::users::UserFull::UserFull(full) = self.invoke(&request).await?;
        let base::UserFull::UserFull(full) = full.full_user;
        let rights = if for_channels { full.bot_broadcast_admin_rights } else { full.bot_group_admin_rights };
        Ok(rights.as_ref().map(ChatPrivileges::parse))
    }
}
