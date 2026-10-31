use crate::Client;
use crate::enums::SentCodeType;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::SentCode;

impl Client {
    pub async fn send_code(&self, phone_number: &str) -> Result<SentCode> {
        let settings = types::CodeSettings {
            allow_flashcall: false,
            current_number: false,
            allow_app_hash: false,
            allow_missed_call: false,
            allow_firebase: false,
            unknown_number: false,
            logout_tokens: None,
            token: None,
            app_sandbox: None,
        };
        let request = functions::auth::SendCode {
            phone_number: phone_number.into(),
            api_id: self.0.api_id,
            api_hash: self.0.api_hash.clone(),
            settings: base::CodeSettings::CodeSettings(Box::new(settings)),
        };
        parse_sent_code(self.invoke_migrating(&request).await?)
    }
}

pub(super) fn parse_sent_code(raw: base::auth::SentCode) -> Result<SentCode> {
    match raw {
        base::auth::SentCode::SentCode(s) => Ok(SentCode {
            kind: SentCodeType::parse(&s.r#type),
            phone_code_hash: s.phone_code_hash,
            next_kind: s.next_type.as_ref().map(SentCodeType::parse_next),
            timeout: s.timeout,
        }),
        base::auth::SentCode::SentCodeSuccess(_) => Err(Error::Unexpected("logged in without a code")),
        base::auth::SentCode::SentCodePaymentRequired(_) => {
            Err(Error::Unexpected("Telegram asks for a payment before it sends the login code"))
        }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use crate::Client;
    use crate::enums::SentCodeType;
    use crate::errors::{Error, Result, RpcError};
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::SentCode;

    fn user(id: i64) -> base::User {
        let mut buf = Vec::new();
        0u32.serialize(&mut buf);
        0u32.serialize(&mut buf);
        id.serialize(&mut buf);
        base::User::User(Box::new(types::User::deserialize_bare(&mut &buf[..]).unwrap()))
    }

    pub(crate) fn sent_code(
        kind: base::auth::SentCodeType,
        hash: &str,
        next_type: Option<base::auth::CodeType>,
        timeout: Option<i32>,
    ) -> Vec<u8> {
        let s = types::auth::SentCode { r#type: kind, phone_code_hash: hash.into(), next_type, timeout };
        base::auth::SentCode::SentCode(Box::new(s)).to_bytes()
    }

    pub(crate) fn sms_code(hash: &str, timeout: Option<i32>) -> Vec<u8> {
        let sms = base::auth::SentCodeType::SentCodeTypeSms(types::auth::SentCodeTypeSms { length: 5 });
        sent_code(sms, hash, None, timeout)
    }

    pub(crate) fn logged_in() -> Vec<u8> {
        let auth = types::auth::Authorization {
            setup_password_required: false,
            otherwise_relogin_days: None,
            tmp_sessions: None,
            future_auth_token: None,
            user: user(7),
        };
        let authorization = base::auth::Authorization::Authorization(Box::new(auth));
        base::auth::SentCode::SentCodeSuccess(Box::new(types::auth::SentCodeSuccess { authorization })).to_bytes()
    }

    pub(crate) fn payment_required() -> Vec<u8> {
        let payment = types::auth::SentCodePaymentRequired {
            store_product: "p".into(),
            phone_code_hash: "h1".into(),
            support_email_address: String::new(),
            support_email_subject: String::new(),
            premium_days: 7,
            currency: "USD".into(),
            amount: 100,
        };
        base::auth::SentCode::SentCodePaymentRequired(Box::new(payment)).to_bytes()
    }

    async fn answer(reply: Result<Vec<u8>>) -> Result<SentCode> {
        let (client, _sent) = Client::scripted(vec![reply]);
        client.send_code("8801").await
    }

    #[tokio::test]
    async fn asks_with_the_app_credentials_and_no_code_options() {
        let (client, mut sent) = Client::recording();
        let _ = client.send_code("8801").await;
        let settings = types::CodeSettings {
            allow_flashcall: false,
            current_number: false,
            allow_app_hash: false,
            allow_missed_call: false,
            allow_firebase: false,
            unknown_number: false,
            logout_tokens: None,
            token: None,
            app_sandbox: None,
        };
        let expected = functions::auth::SendCode {
            phone_number: "8801".into(),
            api_id: client.0.api_id,
            api_hash: client.0.api_hash.clone(),
            settings: base::CodeSettings::CodeSettings(Box::new(settings)),
        };
        assert_eq!(sent.recv().await.unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn a_sent_code_keeps_its_kind_hash_and_timeout() {
        let code = answer(Ok(sms_code("h1", Some(60)))).await.unwrap();
        let expected =
            SentCode { kind: SentCodeType::Sms, phone_code_hash: "h1".into(), next_kind: None, timeout: Some(60) };
        assert_eq!(code, expected);
    }

    #[tokio::test]
    async fn answers_without_a_code_are_errors() {
        let success = answer(Ok(logged_in())).await;
        assert!(matches!(success, Err(Error::Unexpected("logged in without a code"))), "{success:?}");
        let payment = answer(Ok(payment_required())).await;
        let text = "Telegram asks for a payment before it sends the login code";
        assert!(matches!(payment, Err(Error::Unexpected(t)) if t == text), "{payment:?}");
    }

    #[tokio::test]
    async fn refused_numbers_come_back_unchanged() {
        for name in ["PHONE_NUMBER_INVALID", "PHONE_NUMBER_BANNED"] {
            let result = answer(Err(Error::Rpc(RpcError::new(400, name)))).await;
            assert!(matches!(&result, Err(Error::Rpc(e)) if *e == RpcError::new(400, name)), "{result:?}");
        }
    }
}
