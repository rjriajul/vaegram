use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;

impl Client {
    /// Removes the birthday from your profile.
    pub async fn remove_birthday(&self) -> Result<()> {
        accepted(self.invoke(&functions::account::UpdateBirthday { birthday: None }).await?)
    }
}
