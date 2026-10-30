use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn remove_birthday(&self) -> Result<bool> {
        self.invoke(&functions::account::UpdateBirthday { birthday: None }).await
    }
}
