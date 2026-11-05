use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Removes the business account's intro.
    pub async fn delete_business_intro(&self) -> Result<bool> {
        self.invoke(&functions::account::UpdateBusinessIntro { intro: None }).await
    }
}
