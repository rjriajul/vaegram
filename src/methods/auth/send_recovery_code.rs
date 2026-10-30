use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};

impl Client {
    pub async fn send_recovery_code(&self) -> Result<String> {
        let base::auth::PasswordRecovery::PasswordRecovery(r) =
            self.invoke(&functions::auth::RequestPasswordRecovery {}).await?;
        Ok(r.email_pattern)
    }
}
