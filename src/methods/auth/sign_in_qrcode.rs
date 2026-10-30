use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::base::auth::LoginToken as T;
use crate::raw::functions;
use crate::types::LoginToken;
use crate::utils::base64url_encode;

impl Client {
    pub async fn sign_in_qrcode(&self) -> Result<LoginToken> {
        let request = functions::auth::ExportLoginToken {
            api_id: self.0.api_id,
            api_hash: self.0.api_hash.clone(),
            except_ids: Vec::new(),
        };
        let token = match self.invoke(&request).await? {
            T::LoginTokenMigrateTo(m) => {
                self.switch_dc(m.dc_id).await?;
                self.invoke(&functions::auth::ImportLoginToken { token: m.token }).await?
            }
            token => token,
        };
        match token {
            T::LoginToken(t) => Ok(LoginToken::Url { url: login_url(&t.token), expires: t.expires }),
            T::LoginTokenSuccess(s) => Ok(LoginToken::Authorized(self.authorized_user(s.authorization)?)),
            T::LoginTokenMigrateTo(_) => Err(Error::Unexpected("login token migrated twice")),
        }
    }
}

fn login_url(token: &[u8]) -> String {
    format!("tg://login?token={}", base64url_encode(token))
}

#[cfg(test)]
mod tests {
    use super::login_url;

    #[test]
    fn url_is_base64url_without_padding() {
        assert_eq!(login_url(&[0xfb, 0xff, 0x01, 0x02]), "tg://login?token=-_8BAg");
    }
}
