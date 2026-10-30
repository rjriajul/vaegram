use crate::Client;
use crate::errors::Result;

impl Client {
    pub async fn get_contacts_count(&self) -> Result<i32> {
        Ok(self.get_contacts().await?.len() as i32)
    }
}
