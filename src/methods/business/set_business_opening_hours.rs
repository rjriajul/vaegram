use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::BusinessOpeningHours;

impl Client {
    /// Sets the business account's opening hours.
    ///
    /// Fails with `Error::InvalidArgument` if `hours` has no time zone, or an interval that is empty, starts
    /// outside the week, lasts more than a week or ends more than 8 days after Monday 00:00.
    pub async fn set_business_opening_hours(&self, hours: &BusinessOpeningHours) -> Result<()> {
        let business_work_hours = Some(hours.write()?);
        self.invoke(&functions::account::UpdateBusinessWorkHours { business_work_hours }).await?;
        Ok(())
    }
}
