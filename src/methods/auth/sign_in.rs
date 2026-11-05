use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::User;

impl Client {
    /// Signs in with the code Telegram sent.
    ///
    /// Saves the session and returns the signed-in user. Fails with `Error::SignUpRequired` if the number has no
    /// account yet. Call [`sign_up`](Client::sign_up) then.
    pub async fn sign_in(&self, phone_number: &str, phone_code_hash: &str, phone_code: &str) -> Result<base::User> {
        let auth = self.sign_in_raw(phone_number, phone_code_hash, phone_code).await?;
        self.authorized(auth).await
    }

    /// Telegram's answer as it came, so a sign-up still has the terms of service it was shown.
    pub(crate) async fn sign_in_raw(
        &self,
        phone_number: &str,
        phone_code_hash: &str,
        phone_code: &str,
    ) -> Result<base::auth::Authorization> {
        let request = functions::auth::SignIn {
            phone_number: phone_number.into(),
            phone_code_hash: phone_code_hash.into(),
            phone_code: Some(phone_code.into()),
            email_verification: None,
        };
        self.invoke(&request).await
    }

    pub(crate) async fn authorized(&self, auth: base::auth::Authorization) -> Result<base::User> {
        match auth {
            base::auth::Authorization::Authorization(a) => {
                self.set_me(&a.user);
                self.save().await?;
                Ok(a.user)
            }
            base::auth::Authorization::AuthorizationSignUpRequired(_) => Err(Error::SignUpRequired),
        }
    }

    pub(crate) async fn authorized_user(&self, auth: base::auth::Authorization) -> Result<User> {
        Ok(User::from_raw(self, self.authorized(auth).await?))
    }
}
