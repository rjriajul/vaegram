use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::User;
use crate::utils::compute_password_check;

impl Client {
    /// Finishes a login that asked for the two-step verification password.
    ///
    /// Fetches the password settings and answers Telegram's SRP check with `password`. Saves the session
    /// and returns the signed-in user.
    pub async fn check_password(&self, password: &str) -> Result<User> {
        let base::account::Password::Password(p) = self.invoke(&functions::account::GetPassword {}).await?;
        let request = functions::auth::CheckPassword { password: compute_password_check(&p, password)? };
        let auth = self.invoke(&request).await?;
        self.authorized(auth).await
    }
}
