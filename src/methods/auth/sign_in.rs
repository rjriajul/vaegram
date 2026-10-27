use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};

impl Client {
    pub async fn sign_in(&self, phone_number: &str, phone_code_hash: &str, phone_code: &str) -> Result<base::User> {
        let request = functions::auth::SignIn {
            phone_number: phone_number.into(),
            phone_code_hash: phone_code_hash.into(),
            phone_code: Some(phone_code.into()),
            email_verification: None,
        };
        let auth = self.invoke(&request).await?;
        self.authorized(auth)
    }

    pub(crate) fn authorized(&self, auth: base::auth::Authorization) -> Result<base::User> {
        match auth {
            base::auth::Authorization::Authorization(a) => {
                self.set_me(&a.user);
                self.save()?;
                Ok(a.user)
            }
            base::auth::Authorization::AuthorizationSignUpRequired(_) => Err(Error::SignUpRequired),
        }
    }
}
