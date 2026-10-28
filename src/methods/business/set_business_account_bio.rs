use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn set_business_account_bio(&self, business_connection_id: &str, bio: &str) -> Result<()> {
        let request = functions::account::UpdateProfile { first_name: None, last_name: None, about: Some(bio.into()) };
        self.invoke_business(Some(business_connection_id), request).await?;
        Ok(())
    }
}
