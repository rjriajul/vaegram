use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};

impl Client {
    pub async fn sign_in_bot(&self, token: &str) -> Result<base::User> {
        let request = functions::auth::ImportBotAuthorization {
            flags: 0,
            api_id: self.0.api_id,
            api_hash: self.0.api_hash.clone(),
            bot_auth_token: token.into(),
        };
        let auth = self.invoke_migrating(&request).await?;
        self.authorized(auth)
    }
}
