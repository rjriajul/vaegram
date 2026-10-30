use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn delete_business_greeting_message(&self) -> Result<bool> {
        self.invoke(&functions::account::UpdateBusinessGreetingMessage { message: None }).await
    }
}
