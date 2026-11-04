use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Forgets the contact and shipping details saved by earlier orders.
    pub async fn delete_saved_order_info(&self) -> Result<()> {
        self.invoke(&functions::payments::ClearSavedInfo { credentials: false, info: true }).await?;
        Ok(())
    }
}
