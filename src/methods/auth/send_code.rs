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
        match self.invoke_migrating(&request).await? {
            base::auth::SentCode::SentCode(s) => Ok(SentCode {
                kind: SentCodeType::parse(&s.r#type),
                phone_code_hash: s.phone_code_hash,
                timeout: s.timeout,
            }),
            base::auth::SentCode::SentCodeSuccess(_) => Err(Error::Unexpected("logged in without a code")),
            base::auth::SentCode::SentCodePaymentRequired(_) => {
                Err(Error::Unexpected("Telegram asks for a payment before it sends the login code"))
            }
        }
    }
}
