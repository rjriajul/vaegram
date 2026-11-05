use crate::Client;
use crate::errors::Result;

impl Client {
    /// Returns how many contacts you have. It fetches the whole contact list to count it.
    pub async fn get_contacts_count(&self) -> Result<i32> {
        Ok(self.get_contacts().await?.len() as i32)
    }
}
