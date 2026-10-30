use crate::Client;
use crate::crypto::password::verifier;
use crate::crypto::random;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::utils::known_algo;

#[must_use = "a request does nothing until it is awaited"]
pub struct EnableCloudPassword<'a> {
    client: &'a Client,
    password: String,
    hint: Option<String>,
    email: Option<String>,
}

impl Client {
    pub fn enable_cloud_password(&self, password: impl Into<String>) -> EnableCloudPassword<'_> {
        EnableCloudPassword { client: self, password: password.into(), hint: None, email: None }
    }
}

impl EnableCloudPassword<'_> {
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Recovery email. Telegram answers `EMAIL_UNCONFIRMED` and mails a code for `confirm_password_email`.
    pub fn email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }

    async fn send(self) -> Result<bool> {
        let base::account::Password::Password(p) = self.client.invoke(&functions::account::GetPassword {}).await?;
        if p.has_password {
            return Err(Error::InvalidArgument("this account already has a password"));
        }
        let request = functions::account::UpdatePasswordSettings {
            password: base::InputCheckPasswordSRP::InputCheckPasswordEmpty,
            new_settings: new_settings(&p, &self.password, self.hint, self.email, &random())?,
        };
        self.client.invoke(&request).await
    }
}

into_future!(EnableCloudPassword, bool);

type Algo = types::PasswordKdfAlgoSHA256SHA256PBKDF2HMACSHA512iter100000SHA256ModPow;

pub(super) fn new_settings(
    p: &types::account::Password,
    password: &str,
    hint: Option<String>,
    email: Option<String>,
    salt: &[u8; 32],
) -> Result<base::account::PasswordInputSettings> {
    if password.is_empty() {
        return Err(Error::InvalidArgument("the new password is empty"));
    }
    let algo = known_algo(&p.new_algo)?;
    // Telegram leaves it to the client to make salt1 unique for every new password.
    let salt1 = [&algo.salt1[..], salt].concat();
    let hash = verifier(password.as_bytes(), &salt1, &algo.salt2, algo.g, &algo.p).map_err(Error::Security)?;
    let algo = Algo { salt1, salt2: algo.salt2.clone(), g: algo.g, p: algo.p.clone() };
    let settings = types::account::PasswordInputSettings {
        new_algo: Some(base::PasswordKdfAlgo::PasswordKdfAlgoSHA256SHA256PBKDF2HMACSHA512iter100000SHA256ModPow(
            Box::new(algo),
        )),
        new_password_hash: Some(hash.to_vec()),
        hint: Some(hint.unwrap_or_default()),
        email,
        new_secure_settings: None,
    };
    Ok(base::account::PasswordInputSettings::PasswordInputSettings(Box::new(settings)))
}

#[cfg(test)]
mod tests {
    use super::{Algo, new_settings};
    use crate::Client;
    use crate::crypto::password::verifier;
    use crate::crypto::{prime, to_be_padded};
    use crate::errors::{Error, RpcError};
    use crate::raw::{Identifiable, Serializable, functions};
    use crate::raw::{base, types};

    fn password(new_algo: base::PasswordKdfAlgo) -> types::account::Password {
        types::account::Password {
            has_recovery: false,
            has_secure_values: false,
            has_password: false,
            current_algo: None,
            srp_b: None,
            srp_id: None,
            hint: None,
            email_unconfirmed_pattern: None,
            new_algo,
            new_secure_algo: base::SecurePasswordKdfAlgo::SecurePasswordKdfAlgoUnknown,
            secure_random: Vec::new(),
            pending_reset_date: None,
            login_email_pattern: None,
        }
    }

    #[test]
    fn new_password_gets_a_fresh_salt_and_matching_hash() {
        let p = to_be_padded::<256>(&prime::dh_prime()).to_vec();
        let algo = Algo { salt1: b"server".to_vec(), salt2: b"salt two".to_vec(), g: 3, p: p.clone() };
        let account = password(
            base::PasswordKdfAlgo::PasswordKdfAlgoSHA256SHA256PBKDF2HMACSHA512iter100000SHA256ModPow(Box::new(algo)),
        );

        let base::account::PasswordInputSettings::PasswordInputSettings(s) =
            new_settings(&account, "hunter2", None, Some("a@b.c".into()), &[7; 32]).unwrap();
        let Some(base::PasswordKdfAlgo::PasswordKdfAlgoSHA256SHA256PBKDF2HMACSHA512iter100000SHA256ModPow(sent)) =
            &s.new_algo
        else {
            panic!("no new algo")
        };
        assert_eq!(sent.salt1, [&b"server"[..], &[7; 32]].concat());
        assert_eq!((sent.salt2.as_slice(), sent.g, &sent.p), (&b"salt two"[..], 3, &p));
        let expected = verifier(b"hunter2", &sent.salt1, b"salt two", 3, &p).unwrap();
        assert_eq!(s.new_password_hash.as_deref(), Some(&expected[..]));
        assert_eq!((s.hint.as_deref(), s.email.as_deref()), (Some(""), Some("a@b.c")));
    }

    #[test]
    fn new_password_refuses_unknown_algorithms_and_primes() {
        let unknown = password(base::PasswordKdfAlgo::PasswordKdfAlgoUnknown);
        assert!(matches!(new_settings(&unknown, "x", None, None, &[0; 32]), Err(Error::Unexpected(_))));

        let algo = Algo { salt1: Vec::new(), salt2: Vec::new(), g: 3, p: vec![0xff; 256] };
        let bad = password(base::PasswordKdfAlgo::PasswordKdfAlgoSHA256SHA256PBKDF2HMACSHA512iter100000SHA256ModPow(
            Box::new(algo),
        ));
        assert!(matches!(new_settings(&bad, "x", None, None, &[0; 32]), Err(Error::Security(_))));
    }

    fn scripted(password: types::account::Password) -> (Client, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        let reply = base::account::Password::Password(Box::new(password)).to_bytes();
        Client::scripted(vec![Ok(reply)])
    }

    fn known_password() -> types::account::Password {
        let p = to_be_padded::<256>(&prime::dh_prime()).to_vec();
        let algo = Algo { salt1: b"server".to_vec(), salt2: b"salt two".to_vec(), g: 3, p };
        password(base::PasswordKdfAlgo::PasswordKdfAlgoSHA256SHA256PBKDF2HMACSHA512iter100000SHA256ModPow(Box::new(
            algo,
        )))
    }

    #[tokio::test]
    async fn enable_sends_an_empty_check_and_new_settings() {
        let (client, mut sent) = scripted(known_password());
        let _ = client.enable_cloud_password("hunter2").hint("cat").await;
        sent.recv().await.unwrap();
        let body = sent.recv().await.unwrap();
        let expected = [
            &functions::account::UpdatePasswordSettings::CONSTRUCTOR_ID.to_le_bytes()[..],
            &base::InputCheckPasswordSRP::InputCheckPasswordEmpty.to_bytes(),
            &types::account::PasswordInputSettings::CONSTRUCTOR_ID.to_le_bytes(),
        ]
        .concat();
        assert_eq!(body[..12], expected);
    }

    #[tokio::test]
    async fn enable_refuses_when_a_password_exists() {
        let mut account = known_password();
        // has_password shares its flag bit with these, so they must be set too or the reply won't parse.
        account.has_password = true;
        account.current_algo = Some(account.new_algo.clone());
        account.srp_b = Some(vec![0; 256]);
        account.srp_id = Some(1);
        let (client, mut sent) = scripted(account);
        let result = client.enable_cloud_password("hunter2").await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))));
        sent.recv().await.unwrap();
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn an_empty_new_password_is_refused_before_it_is_set() {
        let mut account = known_password();
        let g_b = num_bigint::BigUint::from(3u8).modpow(&num_bigint::BigUint::from(12345u32), &prime::dh_prime());
        let reply = base::account::Password::Password(Box::new(account.clone())).to_bytes();
        account.has_password = true;
        account.current_algo = Some(account.new_algo.clone());
        account.srp_b = Some(to_be_padded::<256>(&g_b).to_vec());
        account.srp_id = Some(1);
        let with_password = base::account::Password::Password(Box::new(account)).to_bytes();
        let refused = || Err(Error::Rpc(RpcError::new(400, "NEW_SETTINGS_INVALID")));

        let (client, mut sent) = Client::scripted(vec![Ok(reply), refused()]);
        let enable = client.enable_cloud_password("").await;
        sent.recv().await.unwrap();
        let enable_sent = sent.try_recv().is_ok();
        let (client, mut sent) = Client::scripted(vec![Ok(with_password), refused()]);
        let change = client.change_cloud_password("hunter2", "").await;
        sent.recv().await.unwrap();
        let change_sent = sent.try_recv().is_ok();

        assert!(matches!(enable, Err(Error::InvalidArgument(_))), "{enable:?}");
        assert!(matches!(change, Err(Error::InvalidArgument(_))), "{change:?}");
        assert_eq!((enable_sent, change_sent), (false, false));
    }

    #[tokio::test]
    async fn remove_sends_an_unknown_algo_with_an_empty_hash() {
        let mut account = known_password();
        let g_b = num_bigint::BigUint::from(3u8).modpow(&num_bigint::BigUint::from(12345u32), &prime::dh_prime());
        account.has_password = true;
        account.current_algo = Some(account.new_algo.clone());
        account.srp_b = Some(to_be_padded::<256>(&g_b).to_vec());
        account.srp_id = Some(1);
        let (client, mut sent) = scripted(account);
        let _ = client.remove_cloud_password("hunter2").await;
        sent.recv().await.unwrap();
        let body = sent.recv().await.unwrap();
        let start = [
            functions::account::UpdatePasswordSettings::CONSTRUCTOR_ID.to_le_bytes(),
            types::InputCheckPasswordSRP::CONSTRUCTOR_ID.to_le_bytes(),
        ]
        .concat();
        assert_eq!(body[..8], start);
        let end = [
            &types::account::PasswordInputSettings::CONSTRUCTOR_ID.to_le_bytes()[..],
            &1u32.to_le_bytes(),
            &base::PasswordKdfAlgo::PasswordKdfAlgoUnknown.to_bytes(),
            &[0; 8],
        ]
        .concat();
        assert_eq!(body[body.len() - 20..], end);
    }
}
