use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::ChatPrivileges;

impl Client {
    /// Returns the default administrator privileges requested when the bot is added to a group, or to a channel
    /// if `for_channels`. `None` if none are set.
    pub async fn get_bot_default_privileges(&self, for_channels: bool) -> Result<Option<ChatPrivileges>> {
        let request = functions::users::GetFullUser { id: base::InputUser::InputUserSelf };
        let base::users::UserFull::UserFull(full) = self.invoke(&request).await?;
        let base::UserFull::UserFull(full) = full.full_user;
        let rights = if for_channels { full.bot_broadcast_admin_rights } else { full.bot_group_admin_rights };
        Ok(rights.as_ref().map(ChatPrivileges::parse))
    }
}
