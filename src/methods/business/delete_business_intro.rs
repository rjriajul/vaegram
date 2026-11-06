use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;

impl Client {
    /// Removes the business account's intro.
    pub async fn delete_business_intro(&self) -> Result<()> {
        accepted(self.invoke(&functions::account::UpdateBusinessIntro { intro: None }).await?)
    }
}
