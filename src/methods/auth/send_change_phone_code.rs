use super::send_code::{code_settings, parse_sent_code};
use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::SentCode;

impl Client {
    /// Telegram refuses it for about a day after a login (`FRESH_CHANGE_PHONE_FORBIDDEN`).
    pub async fn send_change_phone_code(&self, phone_number: &str) -> Result<SentCode> {
        let request =
            functions::account::SendChangePhoneCode { phone_number: phone_number.into(), settings: code_settings() };
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
    async fn sends_the_new_number_with_the_login_settings_and_reads_the_code() {
        let (client, mut sent) = Client::scripted(vec![Ok(sms_code("h3", Some(30)))]);
        let code = client.send_change_phone_code("8802").await.unwrap();
        let expected =
            functions::account::SendChangePhoneCode { phone_number: "8802".into(), settings: code_settings() };
        assert_eq!(sent.recv().await.unwrap(), expected.to_bytes());
        assert_eq!((code.kind, code.phone_code_hash.as_str(), code.timeout), (SentCodeType::Sms, "h3", Some(30)));
    }

    #[tokio::test]
    async fn refusals_come_back_unchanged() {
        for name in ["FRESH_CHANGE_PHONE_FORBIDDEN", "PHONE_NUMBER_OCCUPIED"] {
            let (client, _sent) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, name)))]);
            let result = client.send_change_phone_code("8802").await;
            assert!(matches!(&result, Err(Error::Rpc(e)) if *e == RpcError::new(400, name)), "{result:?}");
        }
    }

    #[test]
    fn the_request_stays_out_of_recordings() {
        assert!(CREDENTIALS.contains(&<functions::account::SendChangePhoneCode as Identifiable>::CONSTRUCTOR_ID));
    }
}
