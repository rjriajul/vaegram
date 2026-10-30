use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn toggle_username(&self, username: impl Into<String>, active: bool) -> Result<bool> {
        self.invoke(&functions::account::ToggleUsername { username: username.into(), active }).await
    }
}
