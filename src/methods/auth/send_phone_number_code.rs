use super::send_code::parse_sent_code;
use crate::Client;
use crate::client::Borrowed;
use crate::enums::PhoneNumberCodeType;
use crate::errors::Result;
use crate::raw::{RemoteCall, base, functions};
use crate::types::{PhoneNumberAuthenticationSettings, SentCode};

#[must_use = "a request does nothing until it is awaited"]
/// Request built by [`Client::send_phone_number_code`].
pub struct SendPhoneNumberCode<'a> {
    client: &'a Client,
    phone_number: String,
    kind: PhoneNumberCodeType,
    settings: PhoneNumberAuthenticationSettings,
    recaptcha_token: Option<String>,
}

impl Client {
    /// Sends a code to `phone_number`, by default for logging in. Aborts a previous verification of the number.
    ///
    /// Awaited without setters it does what [`send_code`](Client::send_code) does. The setters choose what the code
    /// is for ([`kind`](SendPhoneNumberCode::kind)), how Telegram delivers it
    /// ([`settings`](SendPhoneNumberCode::settings)) and a solved reCAPTCHA
    /// ([`recaptcha_token`](SendPhoneNumberCode::recaptcha_token)).
    pub fn send_phone_number_code(&self, phone_number: impl Into<String>) -> SendPhoneNumberCode<'_> {
        SendPhoneNumberCode {
            client: self,
            phone_number: phone_number.into(),
            kind: PhoneNumberCodeType::default(),
            settings: PhoneNumberAuthenticationSettings::default(),
            recaptcha_token: None,
        }
    }
}

impl SendPhoneNumberCode<'_> {
    /// What the code is for. `Change` is refused for about a day after a login (`FRESH_CHANGE_PHONE_FORBIDDEN`).
    pub fn kind(mut self, kind: PhoneNumberCodeType) -> Self {
        self.kind = kind;
        self
    }

    /// Changes how Telegram delivers the code.
    pub fn settings(mut self, settings: PhoneNumberAuthenticationSettings) -> Self {
        self.settings = settings;
        self
    }

    /// The solved reCAPTCHA Telegram asked for before it sends the code.
    pub fn recaptcha_token(mut self, token: impl Into<String>) -> Self {
        self.recaptcha_token = Some(token.into());
        self
    }

    async fn send(self) -> Result<SentCode> {
        let (phone_number, settings) = (self.phone_number, self.settings.write());
        let (client, token) = (self.client, self.recaptcha_token);
        match self.kind {
            PhoneNumberCodeType::Authentication => {
                let request = functions::auth::SendCode {
                    phone_number,
                    api_id: client.0.api_id,
                    api_hash: client.0.api_hash.clone(),
                    settings,
                };
                // Only a login follows the account to another data center.
                call(client, &request, token, true).await
            }
            PhoneNumberCodeType::Change => {
                call(client, &functions::account::SendChangePhoneCode { phone_number, settings }, token, false).await
            }
            PhoneNumberCodeType::Verify => {
                call(client, &functions::account::SendVerifyPhoneCode { phone_number, settings }, token, false).await
            }
        }
    }
}

into_future!(SendPhoneNumberCode, SentCode);

async fn call<R>(client: &Client, request: &R, token: Option<String>, migrating: bool) -> Result<SentCode>
where
    R: RemoteCall<Return = base::auth::SentCode>,
{
    let raw = match (token, migrating) {
        (Some(token), true) => {
            client.invoke_migrating(&functions::InvokeWithReCaptcha { token, query: Borrowed(request) }).await?
        }
        (Some(token), false) => {
            client.invoke(&functions::InvokeWithReCaptcha { token, query: Borrowed(request) }).await?
        }
        (None, true) => client.invoke_migrating(request).await?,
        (None, false) => client.invoke(request).await?,
    };
    parse_sent_code(raw)
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::{PhoneNumberCodeType, SentCodeType};
    use crate::errors::{Error, RpcError};
    use crate::methods::auth::send_code::tests::sms_code;
    use crate::raw::{CREDENTIALS, Identifiable, Serializable, base, functions};
    use crate::types::PhoneNumberAuthenticationSettings;

    fn default_settings() -> base::CodeSettings {
        PhoneNumberAuthenticationSettings::default().write()
    }

    #[tokio::test]
    async fn each_kind_sends_its_function() {
        let (client, mut sent) = Client::scripted(vec![Ok(sms_code("a", None)), Ok(sms_code("b", None))]);
        let kind = |k| client.send_phone_number_code("8802").kind(k);
        let change = kind(PhoneNumberCodeType::Change).await.unwrap();
        let verify = kind(PhoneNumberCodeType::Verify).await.unwrap();
        assert_eq!((change.kind, change.phone_code_hash.as_str()), (SentCodeType::Sms, "a"));
        assert_eq!(verify.phone_code_hash, "b");
        let change =
            functions::account::SendChangePhoneCode { phone_number: "8802".into(), settings: default_settings() };
        let verify =
            functions::account::SendVerifyPhoneCode { phone_number: "8802".into(), settings: default_settings() };
        assert_eq!(sent.recv().await.unwrap(), change.to_bytes());
        assert_eq!(sent.recv().await.unwrap(), verify.to_bytes());
    }

    #[tokio::test]
    async fn custom_settings_are_sent_with_a_login_code() {
        let settings = PhoneNumberAuthenticationSettings::new()
            .allow_flash_call(true)
            .is_current_phone_number(true)
            .authentication_tokens([vec![1, 2]]);
        let (client, mut sent) = Client::recording();
        let _ = client.send_phone_number_code("8801").settings(settings.clone()).await;
        let expected = functions::auth::SendCode {
            phone_number: "8801".into(),
            api_id: client.0.api_id,
            api_hash: client.0.api_hash.clone(),
            settings: settings.write(),
        };
        assert_eq!(sent.recv().await.unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn recaptcha_wraps_the_call() {
        let (client, mut sent) = Client::scripted(vec![Ok(sms_code("a", None))]);
        client.send_phone_number_code("8802").kind(PhoneNumberCodeType::Verify).recaptcha_token("tok").await.unwrap();
        let inner =
            functions::account::SendVerifyPhoneCode { phone_number: "8802".into(), settings: default_settings() };
        let expected = functions::InvokeWithReCaptcha { token: "tok".into(), query: inner };
        assert_eq!(sent.recv().await.unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn only_a_login_code_migrates() {
        // A change or verify code answered with a migration is handed back untouched.
        let migrate = || Err(Error::Rpc(RpcError::parse(303, "PHONE_MIGRATE_2")));
        let (client, mut sent) = Client::scripted(vec![migrate()]);
        let result = client.send_phone_number_code("8802").kind(PhoneNumberCodeType::Change).await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "PHONE_MIGRATE"), "{result:?}");
        sent.recv().await.unwrap();
        assert!(sent.try_recv().is_err());
    }

    #[test]
    fn the_requests_stay_out_of_recordings() {
        assert!(CREDENTIALS.contains(&<functions::account::SendChangePhoneCode as Identifiable>::CONSTRUCTOR_ID));
        assert!(CREDENTIALS.contains(&<functions::account::SendVerifyPhoneCode as Identifiable>::CONSTRUCTOR_ID));
    }
}
