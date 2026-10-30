use super::enable_cloud_password::new_settings;
use crate::Client;
use crate::crypto::random;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::utils::compute_password_check;

#[must_use = "a request does nothing until it is awaited"]
pub struct ChangeCloudPassword<'a> {
    client: &'a Client,
    current: String,
    new: String,
    hint: Option<String>,
}

impl Client {
    pub fn change_cloud_password(
        &self,
        current_password: impl Into<String>,
        new_password: impl Into<String>,
    ) -> ChangeCloudPassword<'_> {
        ChangeCloudPassword { client: self, current: current_password.into(), new: new_password.into(), hint: None }
    }
}

impl ChangeCloudPassword<'_> {
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    async fn send(self) -> Result<bool> {
        let base::account::Password::Password(p) = self.client.invoke(&functions::account::GetPassword {}).await?;
        let request = functions::account::UpdatePasswordSettings {
            password: compute_password_check(&p, &self.current)?,
            new_settings: new_settings(&p, &self.new, self.hint, None, &random())?,
        };
        self.client.invoke(&request).await
    }
}

into_future!(ChangeCloudPassword, bool);
