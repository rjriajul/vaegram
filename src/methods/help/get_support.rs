use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{Support, User};

impl Client {
    pub async fn get_support(&self) -> Result<Support> {
        let base::help::Support::Support(support) = self.invoke(&functions::help::GetSupport {}).await?;
        if matches!(support.user, base::User::UserEmpty(_)) {
            return Err(Error::Unexpected("help.getSupport returned an empty user"));
        }
        Ok(Support { phone_number: support.phone_number, user: User::from_raw(self, support.user) })
    }
}
