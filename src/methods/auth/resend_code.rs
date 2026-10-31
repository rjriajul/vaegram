use super::send_code::parse_sent_code;
use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::SentCode;

impl Client {
    /// Use the `phone_code_hash` it returns from then on.
    pub async fn resend_code(&self, phone_number: &str, phone_code_hash: &str) -> Result<SentCode> {
        let request = functions::auth::ResendCode {
            phone_number: phone_number.into(),
            phone_code_hash: phone_code_hash.into(),
            reason: None,
        };
        parse_sent_code(self.invoke(&request).await?)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::SentCodeType;
    use crate::errors::{Error, Result, RpcError};
    use crate::methods::auth::send_code::tests::{logged_in, payment_required, sent_code, sms_code};
    use crate::raw::{CREDENTIALS, Identifiable, Serializable, base, functions, types};
    use crate::types::SentCode;

    fn rpc(code: i32, name: &str) -> Result<Vec<u8>> {
        Err(Error::Rpc(RpcError::new(code, name)))
    }

    async fn resend(reply: Result<Vec<u8>>) -> Result<SentCode> {
        let (client, _sent) = Client::scripted(vec![reply]);
        client.resend_code("8801", "h1").await
    }

    #[tokio::test]
    async fn sends_the_number_and_hash_as_given() {
        let (client, mut sent) = Client::recording();
        let _ = client.resend_code("8801", "h1").await;
        let expected =
            functions::auth::ResendCode { phone_number: "8801".into(), phone_code_hash: "h1".into(), reason: None };
        assert_eq!(sent.recv().await.unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn the_new_code_brings_its_own_hash_and_next_kind() {
        let call = base::auth::SentCodeType::SentCodeTypeCall(types::auth::SentCodeTypeCall { length: 5 });
        let reply = sent_code(call, "h2", Some(base::auth::CodeType::CodeTypeFlashCall), Some(120));
        let expected = SentCode {
            kind: SentCodeType::Call,
            phone_code_hash: "h2".into(),
            next_kind: Some(SentCodeType::FlashCall),
            timeout: Some(120),
        };
        assert_eq!(resend(Ok(reply)).await.unwrap(), expected);
    }

    #[tokio::test]
    async fn a_resent_hash_is_what_sign_in_sends() {
        let (client, mut sent) = Client::scripted(vec![Ok(sms_code("h1", None)), Ok(sms_code("h2", None))]);
        let first = client.send_code("8801").await.unwrap();
        let second = client.resend_code("8801", &first.phone_code_hash).await.unwrap();
        // Nothing answers the sign-in; only its body matters here.
        let _ = client.sign_in("8801", &second.phone_code_hash, "22222").await;
        let bodies: Vec<_> = std::iter::from_fn(|| sent.try_recv().ok()).collect();
        let sign_in = functions::auth::SignIn {
            phone_number: "8801".into(),
            phone_code_hash: "h2".into(),
            phone_code: Some("22222".into()),
            email_verification: None,
        };
        assert_eq!(bodies.len(), 3);
        assert_eq!(bodies[2], sign_in.to_bytes());
    }

    #[tokio::test]
    async fn resend_errors_come_back_unchanged() {
        for name in ["PHONE_CODE_EXPIRED", "PHONE_CODE_HASH_EMPTY", "SEND_CODE_UNAVAILABLE"] {
            let result = resend(rpc(400, name)).await;
            assert!(matches!(&result, Err(Error::Rpc(e)) if *e == RpcError::new(400, name)), "{result:?}");
        }
    }

    #[tokio::test]
    async fn sign_in_errors_after_a_resend_come_back_unchanged() {
        for (code, name) in [(400, "PHONE_CODE_EXPIRED"), (400, "PHONE_CODE_INVALID"), (401, "SESSION_PASSWORD_NEEDED")]
        {
            let (client, _sent) = Client::scripted(vec![Ok(sms_code("h2", None)), rpc(code, name)]);
            let code_hash = client.resend_code("8801", "h1").await.unwrap().phone_code_hash;
            let result = client.sign_in("8801", &code_hash, "22222").await;
            assert!(matches!(&result, Err(Error::Rpc(e)) if *e == RpcError::new(code, name)), "{result:?}");
        }
    }

    #[tokio::test]
    async fn answers_without_a_code_are_errors() {
        assert!(matches!(resend(Ok(logged_in())).await, Err(Error::Unexpected("logged in without a code"))));
        let text = "Telegram asks for a payment before it sends the login code";
        assert!(matches!(resend(Ok(payment_required())).await, Err(Error::Unexpected(t)) if t == text));
    }

    #[test]
    fn the_request_and_its_answer_stay_out_of_recordings() {
        assert!(CREDENTIALS.contains(&<functions::auth::ResendCode as Identifiable>::CONSTRUCTOR_ID));
        assert!(CREDENTIALS.contains(&<types::auth::SentCode as Identifiable>::CONSTRUCTOR_ID));
    }
}
