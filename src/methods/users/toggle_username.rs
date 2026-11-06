use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;

impl Client {
    /// Turns one of your usernames on or off.
    pub async fn toggle_username(&self, username: impl Into<String>, active: bool) -> Result<()> {
        accepted(self.invoke(&functions::account::ToggleUsername { username: username.into(), active }).await?)
    }
}
