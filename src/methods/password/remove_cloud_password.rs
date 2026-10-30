use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::utils::compute_password_check;

impl Client {
    pub async fn remove_cloud_password(&self, password: &str) -> Result<bool> {
        let base::account::Password::Password(p) = self.invoke(&functions::account::GetPassword {}).await?;
        let settings = types::account::PasswordInputSettings {
            new_algo: Some(base::PasswordKdfAlgo::PasswordKdfAlgoUnknown),
            new_password_hash: Some(Vec::new()),
            hint: Some(String::new()),
            email: None,
            new_secure_settings: None,
        };
        let request = functions::account::UpdatePasswordSettings {
            password: compute_password_check(&p, password)?,
            new_settings: base::account::PasswordInputSettings::PasswordInputSettings(Box::new(settings)),
        };
        self.invoke(&request).await
    }
}
