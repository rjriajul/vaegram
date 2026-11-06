use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Sets the username of the business account behind a connection.
    pub async fn set_business_account_username(
        &self,
        business_connection_id: impl Into<String>,
        username: &str,
    ) -> Result<()> {
        let business_connection_id: String = business_connection_id.into();
        let request = functions::account::UpdateUsername { username: username.into() };
        self.invoke_business(Some(&business_connection_id), request).await?;
        Ok(())
    }
}
