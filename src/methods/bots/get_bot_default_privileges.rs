use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::ChatPrivileges;

impl Client {
    /// Returns the administrator privileges this bot asks for when added to a group.
    ///
    /// With `for_channels`, returns the ones for channels instead. `None` if none are set.
    pub async fn get_bot_default_privileges(&self, for_channels: bool) -> Result<Option<ChatPrivileges>> {
        // The Bot API keeps them out of the bot's full user, which TL reads them from.
        #[cfg(feature = "bot-api")]
        if let Some(api) = self.0.bot_api.as_deref() {
            return Ok(api.default_admin_rights(for_channels).await?.as_ref().map(ChatPrivileges::parse));
        }
        let request = functions::users::GetFullUser { id: base::InputUser::InputUserSelf };
        let base::users::UserFull::UserFull(full) = self.invoke(&request).await?;
        let base::UserFull::UserFull(full) = full.full_user;
        let rights = if for_channels { full.bot_broadcast_admin_rights } else { full.bot_group_admin_rights };
        Ok(rights.as_ref().map(ChatPrivileges::parse))
    }
}
