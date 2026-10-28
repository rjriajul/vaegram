use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn set_business_account_username(&self, business_connection_id: &str, username: &str) -> Result<()> {
        let request = functions::account::UpdateUsername { username: username.into() };
        self.invoke_business(Some(business_connection_id), request).await?;
        Ok(())
    }
}
