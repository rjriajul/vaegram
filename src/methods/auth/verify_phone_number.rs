use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Checks the code `send_verify_phone_code` sent, with the hash it returned.
    pub async fn verify_phone_number(&self, phone_number: &str, phone_code_hash: &str, phone_code: &str) -> Result<()> {
        let request = functions::account::VerifyPhone {
            phone_number: phone_number.into(),
            phone_code_hash: phone_code_hash.into(),
            phone_code: phone_code.into(),
        };
        self.invoke(&request).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::{CREDENTIALS, Identifiable, Serializable, functions};

    #[tokio::test]
    async fn sends_the_number_the_hash_and_the_code() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes())]);
        client.verify_phone_number("8802", "h3", "12345").await.unwrap();
        let expected = functions::account::VerifyPhone {
            phone_number: "8802".into(),
            phone_code_hash: "h3".into(),
            phone_code: "12345".into(),
        };
        assert_eq!(sent.recv().await.unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn code_errors_come_back_unchanged() {
        for name in ["PHONE_CODE_EXPIRED", "PHONE_CODE_INVALID"] {
            let (client, _sent) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, name)))]);
            let result = client.verify_phone_number("8802", "h3", "12345").await;
            assert!(matches!(&result, Err(Error::Rpc(e)) if *e == RpcError::new(400, name)), "{result:?}");
        }
    }

    #[test]
    fn the_request_stays_out_of_recordings() {
        assert!(CREDENTIALS.contains(&<functions::account::VerifyPhone as Identifiable>::CONSTRUCTOR_ID));
    }
}
