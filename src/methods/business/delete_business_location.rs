use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;

impl Client {
    /// Removes the business account's location.
    pub async fn delete_business_location(&self) -> Result<()> {
        accepted(self.invoke(&functions::account::UpdateBusinessLocation { geo_point: None, address: None }).await?)
    }
}
