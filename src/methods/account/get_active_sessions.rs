use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::ActiveSessions;

impl Client {
    /// Returns the devices logged in to your account.
    pub async fn get_active_sessions(&self) -> Result<ActiveSessions> {
        Ok(ActiveSessions::parse(self.invoke(&functions::account::GetAuthorizations {}).await?))
    }
}
