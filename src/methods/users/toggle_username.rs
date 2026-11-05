use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Turns one of your usernames on or off. Returns whether Telegram accepted the change.
    pub async fn toggle_username(&self, username: impl Into<String>, active: bool) -> Result<bool> {
        self.invoke(&functions::account::ToggleUsername { username: username.into(), active }).await
    }
}
