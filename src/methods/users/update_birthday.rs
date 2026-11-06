use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::{base, functions, types};

impl Client {
    /// Sets the birthday on your profile.
    pub async fn update_birthday(&self, day: i32, month: i32, year: Option<i32>) -> Result<()> {
        let birthday = base::Birthday::Birthday(Box::new(types::Birthday { day, month, year }));
        accepted(self.invoke(&functions::account::UpdateBirthday { birthday: Some(birthday) }).await?)
    }
}
