use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Sets the bio of the business account behind a connection.
    pub async fn set_business_account_bio(&self, business_connection_id: impl Into<String>, bio: &str) -> Result<()> {
        let business_connection_id: String = business_connection_id.into();
        let request = functions::account::UpdateProfile { first_name: None, last_name: None, about: Some(bio.into()) };
        self.invoke_business(Some(&business_connection_id), request).await?;
        Ok(())
    }
}
