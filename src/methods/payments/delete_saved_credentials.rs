use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Forgets the cards providers saved for this account.
    pub async fn delete_saved_credentials(&self) -> Result<()> {
        self.invoke(&functions::payments::ClearSavedInfo { credentials: true, info: false }).await?;
        Ok(())
    }
}
