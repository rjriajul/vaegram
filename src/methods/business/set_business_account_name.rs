use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn set_business_account_name(
        &self,
        business_connection_id: &str,
        first_name: &str,
        last_name: &str,
    ) -> Result<()> {
        // An empty last name is how Telegram clears it.
        let request = functions::account::UpdateProfile {
            first_name: Some(first_name.into()),
            last_name: Some(last_name.into()),
            about: None,
        };
        self.invoke_business(Some(business_connection_id), request).await?;
        Ok(())
    }
}
