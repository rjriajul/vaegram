use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::User;

impl Client {
    pub async fn change_phone_number(
        &self,
        phone_number: &str,
        phone_code_hash: &str,
        phone_code: &str,
    ) -> Result<User> {
        let request = functions::account::ChangePhone {
            phone_number: phone_number.into(),
            phone_code_hash: phone_code_hash.into(),
            phone_code: phone_code.into(),
        };
        Ok(User::from_raw(self, self.invoke(&request).await?))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::methods::auth::send_code::tests::user;
    use crate::raw::{CREDENTIALS, Identifiable, Serializable, functions};

    #[tokio::test]
    async fn sends_the_code_and_returns_the_account() {
        let (client, mut sent) = Client::scripted(vec![Ok(user(7).to_bytes())]);
        client.0.me.lock().unwrap().id = 7;
        let account = client.change_phone_number("8802", "h3", "12345").await.unwrap();
        let expected = functions::account::ChangePhone {
            phone_number: "8802".into(),
            phone_code_hash: "h3".into(),
            phone_code: "12345".into(),
        };
        assert_eq!(sent.recv().await.unwrap(), expected.to_bytes());
        assert_eq!(account.id, 7);
        // The account stays the same one; only its number changed.
        assert_eq!(client.0.me.lock().unwrap().id, 7);
    }

    #[tokio::test]
    async fn code_errors_come_back_unchanged() {
        for name in ["PHONE_CODE_EXPIRED", "PHONE_CODE_INVALID"] {
            let (client, _sent) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, name)))]);
            let result = client.change_phone_number("8802", "h3", "12345").await;
            assert!(matches!(&result, Err(Error::Rpc(e)) if *e == RpcError::new(400, name)), "{result:?}");
        }
    }

    #[test]
    fn the_request_stays_out_of_recordings() {
        assert!(CREDENTIALS.contains(&<functions::account::ChangePhone as Identifiable>::CONSTRUCTOR_ID));
    }
}
