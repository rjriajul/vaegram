use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::User;

impl Client {
    pub async fn set_username(&self, username: impl Into<String>) -> Result<User> {
        let user = self.invoke(&functions::account::UpdateUsername { username: username.into() }).await?;
        Ok(User::from_raw(self, user))
    }
}
