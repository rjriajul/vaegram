use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Removes the business account's opening hours.
    pub async fn delete_business_opening_hours(&self) -> Result<()> {
        self.invoke(&functions::account::UpdateBusinessWorkHours { business_work_hours: None }).await?;
        Ok(())
    }
}
