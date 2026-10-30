use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn delete_business_location(&self) -> Result<bool> {
        self.invoke(&functions::account::UpdateBusinessLocation { geo_point: None, address: None }).await
    }
}
