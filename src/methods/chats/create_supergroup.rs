use crate::Client;
use crate::errors::Result;
use crate::types::Chat;

impl Client {
    pub async fn create_supergroup(&self, title: impl Into<String>, description: impl Into<String>) -> Result<Chat> {
        super::create_channel::create(self, title.into(), description.into(), false).await
    }
}
