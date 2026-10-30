use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn delete_business_opening_hours(&self) -> Result<bool> {
        self.invoke(&functions::account::UpdateBusinessWorkHours { business_work_hours: None }).await
    }
}
