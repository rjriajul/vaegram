use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::User;

impl Client {
    /// Signs in as a bot with a token from @BotFather.
    ///
    /// Follows a migration to another data center and saves the session.
    pub async fn sign_in_bot(&self, token: &str) -> Result<User> {
        let auth = self.sign_in_bot_raw(token).await?;
        self.authorized(auth).await
    }

    pub(crate) async fn sign_in_bot_raw(&self, token: &str) -> Result<base::auth::Authorization> {
        let request = functions::auth::ImportBotAuthorization {
            flags: 0,
            api_id: self.0.api_id,
            api_hash: self.0.api_hash.clone(),
            bot_auth_token: token.into(),
        };
        self.invoke_migrating(&request).await
    }
}
