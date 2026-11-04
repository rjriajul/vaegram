use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{TonWalletOwnershipProof, TonWalletState};
use crate::utils::compute_password_check;

/// The cloud password check, fetched only when a password was given.
pub(super) async fn password_check(
    client: &Client,
    password: Option<&str>,
) -> Result<Option<base::InputCheckPasswordSRP>> {
    let Some(password) = password else { return Ok(None) };
    let base::account::Password::Password(p) = client.invoke(&functions::account::GetPassword {}).await?;
    Ok(Some(compute_password_check(&p, password)?))
}

#[must_use = "a request does nothing until it is awaited"]
pub struct ReplaceTonWallet<'a> {
    client: &'a Client,
    proof: TonWalletOwnershipProof,
    password: Option<String>,
    anchor_public_key: Option<Vec<u8>>,
}

impl Client {
    /// Swaps the account's wallet for one you hold the key to. Irreversible: the old wallet is detached.
    pub fn replace_ton_wallet(&self, proof: TonWalletOwnershipProof) -> ReplaceTonWallet<'_> {
        ReplaceTonWallet { client: self, proof, password: None, anchor_public_key: None }
    }
}

impl ReplaceTonWallet<'_> {
    /// The cloud password, for accounts that have one.
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    pub fn anchor_public_key(mut self, key: impl Into<Vec<u8>>) -> Self {
        self.anchor_public_key = Some(key.into());
        self
    }

    fn request(self, password: Option<base::InputCheckPasswordSRP>) -> functions::wallet::ReplaceWallet {
        let imported = crate::raw::types::InputWalletImported {
            public_key: self.proof.public_key.clone(),
            anchor_public_key: self.anchor_public_key,
            proof: self.proof.to_raw(),
        };
        functions::wallet::ReplaceWallet {
            wallet: base::InputWalletReplacement::InputWalletImported(Box::new(imported)),
            password,
        }
    }

    async fn send(self) -> Result<TonWalletState> {
        let password = password_check(self.client, self.password.as_deref()).await?;
        let client = self.client;
        let state = client.invoke(&self.request(password)).await?;
        Ok(TonWalletState::parse(client, &state))
    }
}

into_future!(ReplaceTonWallet, TonWalletState);

#[cfg(test)]
pub(super) mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::functions::wallet::ReplaceWallet;
    use crate::raw::{Serializable, base, types};
    use crate::types::TonWalletOwnershipProof;

    /// An `account.getPassword` answer for an account with a cloud password, which SRP can be computed from.
    pub(in crate::methods::wallet) fn password_reply() -> Vec<u8> {
        use crate::crypto::{prime, to_be_padded};
        type Algo = types::PasswordKdfAlgoSHA256SHA256PBKDF2HMACSHA512iter100000SHA256ModPow;
        let p = to_be_padded::<256>(&prime::dh_prime()).to_vec();
        let algo =
            base::PasswordKdfAlgo::PasswordKdfAlgoSHA256SHA256PBKDF2HMACSHA512iter100000SHA256ModPow(Box::new(Algo {
                salt1: b"server".to_vec(),
                salt2: b"salt two".to_vec(),
                g: 3,
                p,
            }));
        let g_b = num_bigint::BigUint::from(3u8).modpow(&num_bigint::BigUint::from(12345u32), &prime::dh_prime());
        let account = types::account::Password {
            has_recovery: false,
            has_secure_values: false,
            has_password: true,
            current_algo: Some(algo.clone()),
            srp_b: Some(to_be_padded::<256>(&g_b).to_vec()),
            srp_id: Some(1),
            hint: None,
            email_unconfirmed_pattern: None,
            new_algo: algo,
            new_secure_algo: base::SecurePasswordKdfAlgo::SecurePasswordKdfAlgoUnknown,
            secure_random: Vec::new(),
            pending_reset_date: None,
            login_email_pattern: None,
        };
        base::account::Password::Password(Box::new(account)).to_bytes()
    }

    /// Whether a serialized `wallet.replaceWallet` carries the password (flag bit 0).
    pub(in crate::methods::wallet) fn sends_password(body: &[u8]) -> bool {
        body[..4] == <ReplaceWallet as crate::raw::Identifiable>::CONSTRUCTOR_ID.to_le_bytes() && body[4] & 1 == 1
    }

    fn proof() -> TonWalletOwnershipProof {
        TonWalletOwnershipProof { public_key: vec![1, 2], timestamp: 10, signature: vec![3] }
    }

    #[tokio::test]
    async fn replace_sends_imported_wallet_with_proof() {
        let reply = base::WalletState::WalletStateEmpty(types::WalletStateEmpty { creating: true }).to_bytes();
        let (client, mut rx) = Client::scripted(vec![Ok(reply)]);
        let state = client.replace_ton_wallet(proof()).anchor_public_key(vec![9]).await.unwrap();
        assert!(state.is_being_created);
        let imported = types::InputWalletImported {
            public_key: vec![1, 2],
            anchor_public_key: Some(vec![9]),
            proof: base::WalletOwnershipProof::WalletOwnershipProof(Box::new(types::WalletOwnershipProof {
                timestamp: 10,
                signature: vec![3],
            })),
        };
        let expected = ReplaceWallet {
            wallet: base::InputWalletReplacement::InputWalletImported(Box::new(imported)),
            password: None,
        };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
        // Without a password nothing else is sent.
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn password_is_checked_first() {
        let (client, mut rx) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, "PASSWORD_HASH_INVALID")))]);
        let result = client.replace_ton_wallet(proof()).password("hunter2").await;
        assert!(matches!(&result, Err(e) if e.is("PASSWORD_HASH_INVALID")), "{result:?}");
        rx.try_recv().unwrap();
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_password_is_fetched_then_sent_with_the_wallet() {
        let state = base::WalletState::WalletStateEmpty(types::WalletStateEmpty { creating: false }).to_bytes();
        let (client, mut rx) = Client::scripted(vec![Ok(password_reply()), Ok(state)]);
        client.replace_ton_wallet(proof()).password("hunter2").await.unwrap();
        let first = rx.try_recv().unwrap();
        assert_eq!(first, crate::raw::functions::account::GetPassword {}.to_bytes());
        assert!(sends_password(&rx.try_recv().unwrap()));
        assert!(rx.try_recv().is_err());
    }
}
