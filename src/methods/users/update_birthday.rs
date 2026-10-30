use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};

impl Client {
    pub async fn update_birthday(&self, day: i32, month: i32, year: Option<i32>) -> Result<bool> {
        let birthday = base::Birthday::Birthday(Box::new(types::Birthday { day, month, year }));
        self.invoke(&functions::account::UpdateBirthday { birthday: Some(birthday) }).await
    }
}
