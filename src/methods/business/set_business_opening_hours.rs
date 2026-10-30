use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::BusinessOpeningHours;

impl Client {
    pub async fn set_business_opening_hours(&self, hours: &BusinessOpeningHours) -> Result<bool> {
        let business_work_hours = Some(hours.write()?);
        self.invoke(&functions::account::UpdateBusinessWorkHours { business_work_hours }).await
    }
}
