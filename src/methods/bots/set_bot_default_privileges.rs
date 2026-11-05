use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::ChatPrivileges;

impl Client {
    /// Sets the default administrator privileges requested when the bot is added to a group, or to a channel
    /// if `for_channels`.
    pub async fn set_bot_default_privileges(&self, privileges: &ChatPrivileges, for_channels: bool) -> Result<bool> {
        let admin_rights = privileges.write();
        if for_channels {
            self.invoke(&functions::bots::SetBotBroadcastDefaultAdminRights { admin_rights }).await
        } else {
            self.invoke(&functions::bots::SetBotGroupDefaultAdminRights { admin_rights }).await
        }
    }
}
