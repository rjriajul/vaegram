use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};

const MAX_MIGRATIONS: usize = 3;

impl Client {
    pub async fn sign_in_bot(&self, token: &str) -> Result<base::auth::Authorization> {
        let mut migrations = 0;
        loop {
            let request = functions::auth::ImportBotAuthorization {
                flags: 0,
                api_id: self.0.api_id,
                api_hash: self.0.api_hash.clone(),
                bot_auth_token: token.into(),
            };
            match self.invoke(&request).await {
                Ok(auth) => {
                    if let base::auth::Authorization::Authorization(a) = &auth {
                        self.set_me(&a.user);
                    }
                    self.save()?;
                    return Ok(auth);
                }
                Err(Error::Rpc(e)) if e.code == 303 && migrations < MAX_MIGRATIONS => {
                    let dc_id = e.value.ok_or(Error::Unexpected("migrate error without a DC number"))?;
                    self.switch_dc(dc_id as i32).await?;
                    migrations += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }
}
