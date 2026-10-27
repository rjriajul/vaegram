use crate::Client;
use crate::crypto::password::Srp;
use crate::crypto::random;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};

impl Client {
    pub async fn check_password(&self, password: &str) -> Result<base::User> {
        let base::account::Password::Password(p) = self.invoke(&functions::account::GetPassword {}).await?;
        let (Some(algo), Some(g_b), Some(srp_id)) = (&p.current_algo, &p.srp_b, p.srp_id) else {
            return Err(Error::Unexpected("this account has no password"));
        };
        let base::PasswordKdfAlgo::PasswordKdfAlgoSHA256SHA256PBKDF2HMACSHA512iter100000SHA256ModPow(algo) = algo
        else {
            return Err(Error::Unexpected("unknown password algorithm; update vaegram"));
        };
        let srp = Srp { salt1: &algo.salt1, salt2: &algo.salt2, g: algo.g, p: &algo.p, g_b };
        let (a, m1) = srp.check(password.as_bytes(), &random()).map_err(Error::Security)?;
        let check = types::InputCheckPasswordSRP { srp_id, a: a.to_vec(), m1: m1.to_vec() };
        let request = functions::auth::CheckPassword {
            password: base::InputCheckPasswordSRP::InputCheckPasswordSRP(Box::new(check)),
        };
        let auth = self.invoke(&request).await?;
        self.authorized(auth)
    }
}
