use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};

impl Client {
    /// Mails a password recovery code to the account's recovery email.
    ///
    /// Returns the masked address the code went to. Pass the code to [`recover_password`](Client::recover_password).
    pub async fn send_recovery_code(&self) -> Result<String> {
        let base::auth::PasswordRecovery::PasswordRecovery(r) =
            self.invoke(&functions::auth::RequestPasswordRecovery {}).await?;
        Ok(r.email_pattern)
    }
}
