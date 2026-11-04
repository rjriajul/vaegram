use super::send_code::{code_settings, parse_sent_code};
use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::SentCode;

impl Client {
    /// Sends a code proving the account owns `phone_number`, for services such as Telegram Passport that ask for
    /// a verified number; `verify_phone_number` checks it.
    pub async fn send_verify_phone_code(&self, phone_number: &str) -> Result<SentCode> {
        let request =
            functions::account::SendVerifyPhoneCode { phone_number: phone_number.into(), settings: code_settings() };
        parse_sent_code(self.invoke(&request).await?)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::SentCodeType;
    use crate::errors::{Error, RpcError};
    use crate::methods::auth::send_code::code_settings;
    use crate::methods::auth::send_code::tests::sms_code;
    use crate::raw::{CREDENTIALS, Identifiable, Serializable, functions};

    #[tokio::test]
    async fn sends_the_number_with_the_login_settings_and_reads_the_code() {
        let (client, mut sent) = Client::scripted(vec![Ok(sms_code("h3", Some(30)))]);
        let code = client.send_verify_phone_code("8802").await.unwrap();
        let expected =
            functions::account::SendVerifyPhoneCode { phone_number: "8802".into(), settings: code_settings() };
        assert_eq!(sent.recv().await.unwrap(), expected.to_bytes());
        assert_eq!((code.kind, code.phone_code_hash.as_str(), code.timeout), (SentCodeType::Sms, "h3", Some(30)));
    }

    #[tokio::test]
    async fn refusals_come_back_unchanged() {
        for name in ["PHONE_NUMBER_INVALID", "PHONE_NUMBER_BANNED"] {
            let (client, _sent) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, name)))]);
            let result = client.send_verify_phone_code("8802").await;
            assert!(matches!(&result, Err(Error::Rpc(e)) if *e == RpcError::new(400, name)), "{result:?}");
        }
    }

    #[test]
    fn the_request_stays_out_of_recordings() {
        assert!(CREDENTIALS.contains(&<functions::account::SendVerifyPhoneCode as Identifiable>::CONSTRUCTOR_ID));
    }
}
