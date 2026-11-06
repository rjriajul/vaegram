use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::{base, functions, types};

impl Client {
    /// Sets how many days of inactivity pass before Telegram deletes your account.
    pub async fn set_account_ttl(&self, days: i32) -> Result<()> {
        let ttl = base::AccountDaysTTL::AccountDaysTTL(types::AccountDaysTTL { days });
        accepted(self.invoke(&functions::account::SetAccountTTL { ttl }).await?)
    }
}
