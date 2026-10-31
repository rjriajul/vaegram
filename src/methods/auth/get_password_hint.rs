use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};

impl Client {
    pub async fn get_password_hint(&self) -> Result<Option<String>> {
        let base::account::Password::Password(p) = self.invoke(&functions::account::GetPassword {}).await?;
        Ok(p.hint.filter(|h| !h.is_empty()))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, Result, RpcError};
    use crate::raw::{CREDENTIALS, Identifiable, Serializable, base, functions, types};

    fn password(hint: Option<&str>) -> Vec<u8> {
        let password = types::account::Password {
            has_recovery: false,
            has_secure_values: false,
            // The flag shares its bit with the SRP fields, which these answers leave out.
            has_password: false,
            current_algo: None,
            srp_b: None,
            srp_id: None,
            hint: hint.map(Into::into),
            email_unconfirmed_pattern: None,
            new_algo: base::PasswordKdfAlgo::PasswordKdfAlgoUnknown,
            new_secure_algo: base::SecurePasswordKdfAlgo::SecurePasswordKdfAlgoUnknown,
            secure_random: Vec::new(),
            pending_reset_date: None,
            login_email_pattern: None,
        };
        base::account::Password::Password(Box::new(password)).to_bytes()
    }

    async fn hint(reply: Result<Vec<u8>>) -> Result<Option<String>> {
        let (client, _sent) = Client::scripted(vec![reply]);
        client.get_password_hint().await
    }

    #[tokio::test]
    async fn asks_for_the_password_settings() {
        let (client, mut sent) = Client::recording();
        let _ = client.get_password_hint().await;
        assert_eq!(sent.recv().await.unwrap(), functions::account::GetPassword {}.to_bytes());
    }

    #[tokio::test]
    async fn an_empty_or_missing_hint_is_none() {
        assert_eq!(hint(Ok(password(Some("cat")))).await.unwrap().as_deref(), Some("cat"));
        // Telegram sends an empty hint for a password set without one; there is nothing to show.
        assert_eq!(hint(Ok(password(Some("")))).await.unwrap(), None);
        // No password at all, so no hint.
        assert_eq!(hint(Ok(password(None))).await.unwrap(), None);
    }

    #[tokio::test]
    async fn errors_come_back_unchanged() {
        let result = hint(Err(Error::Rpc(RpcError::new(400, "X")))).await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if *e == RpcError::new(400, "X")), "{result:?}");
    }

    #[test]
    fn the_request_stays_out_of_recordings() {
        assert!(CREDENTIALS.contains(&<functions::account::GetPassword as Identifiable>::CONSTRUCTOR_ID));
    }
}
