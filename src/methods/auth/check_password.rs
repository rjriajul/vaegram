use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::utils::compute_password_check;

impl Client {
    pub async fn check_password(&self, password: &str) -> Result<base::User> {
        let base::account::Password::Password(p) = self.invoke(&functions::account::GetPassword {}).await?;
        let request = functions::auth::CheckPassword { password: compute_password_check(&p, password)? };
        let auth = self.invoke(&request).await?;
        self.authorized(auth)
    }
}
