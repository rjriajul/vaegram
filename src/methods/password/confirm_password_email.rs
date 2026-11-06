use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;

impl Client {
    /// Confirms the recovery email with the code Telegram mailed to it.
    pub async fn confirm_password_email(&self, code: &str) -> Result<()> {
        accepted(self.invoke(&functions::account::ConfirmPasswordEmail { code: code.into() }).await?)
    }
}
