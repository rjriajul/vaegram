use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn delete_business_away_message(&self) -> Result<bool> {
        self.invoke(&functions::account::UpdateBusinessAwayMessage { message: None }).await
    }
}
