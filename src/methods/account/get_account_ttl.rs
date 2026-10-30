use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};

impl Client {
    pub async fn get_account_ttl(&self) -> Result<i32> {
        let base::AccountDaysTTL::AccountDaysTTL(ttl) = self.invoke(&functions::account::GetAccountTTL {}).await?;
        Ok(ttl.days)
    }
}
