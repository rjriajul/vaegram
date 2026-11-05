use crate::Client;
use crate::errors::Result;

impl Client {
    /// Returns how many contacts you have.
    ///
    /// Fetches the whole contact list through [`Client::get_contacts`] and counts it.
    pub async fn get_contacts_count(&self) -> Result<i32> {
        Ok(self.get_contacts().await?.len() as i32)
    }
}
