use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Removes the business account's location.
    pub async fn delete_business_location(&self) -> Result<()> {
        self.invoke(&functions::account::UpdateBusinessLocation { geo_point: None, address: None }).await?;
        Ok(())
    }
}
