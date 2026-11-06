use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Removes the business account's away message.
    pub async fn delete_business_away_message(&self) -> Result<()> {
        self.invoke(&functions::account::UpdateBusinessAwayMessage { message: None }).await?;
        Ok(())
    }
}
