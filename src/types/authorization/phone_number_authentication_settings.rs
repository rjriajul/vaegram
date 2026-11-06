use std::fmt;

use crate::raw::{base, types};

/// Firebase delivery of the code, for official apps' use.
#[derive(Clone, PartialEq, Eq)]
pub enum FirebaseAuthenticationSettings {
    /// Android device.
    Android,
    /// iOS device.
    Ios {
        /// Push token of the device.
        device_token: String,
        /// The app uses the sandbox push environment.
        is_app_sandbox: bool,
    },
}

/// How Telegram may deliver a phone number code. The default asks for nothing special.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct PhoneNumberAuthenticationSettings {
    /// Telegram may deliver the code as a flash call.
    pub allow_flash_call: bool,
    /// Telegram may deliver the code as a missed call.
    pub allow_missed_call: bool,
    /// The number is the one this device is already using.
    pub is_current_phone_number: bool,
    /// The device has a SIM card but it can't be checked whether its number matches.
    pub has_unknown_phone_number: bool,
    /// Telegram may append an app hash to the SMS so Android's SMS Retriever API can read the code.
    pub allow_sms_retriever_api: bool,
    /// Lets Telegram deliver the code through Firebase. `None` turns Firebase delivery off.
    pub firebase_authentication_settings: Option<FirebaseAuthenticationSettings>,
    /// Tokens (up to 20) received in previously logged out sessions.
    pub authentication_tokens: Vec<Vec<u8>>,
}

// The device token and logout tokens identify the device and its old sessions, so they stay out of logs.
impl fmt::Debug for FirebaseAuthenticationSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Android => f.write_str("Android"),
            Self::Ios { is_app_sandbox, .. } => {
                f.debug_struct("Ios").field("is_app_sandbox", is_app_sandbox).finish_non_exhaustive()
            }
        }
    }
}

impl fmt::Debug for PhoneNumberAuthenticationSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PhoneNumberAuthenticationSettings")
            .field("allow_flash_call", &self.allow_flash_call)
            .field("allow_missed_call", &self.allow_missed_call)
            .field("is_current_phone_number", &self.is_current_phone_number)
            .field("has_unknown_phone_number", &self.has_unknown_phone_number)
            .field("allow_sms_retriever_api", &self.allow_sms_retriever_api)
            .field("firebase_authentication_settings", &self.firebase_authentication_settings)
            .field("authentication_tokens", &self.authentication_tokens.len())
            .finish()
    }
}

impl PhoneNumberAuthenticationSettings {
    /// Asks for nothing special, like `Default`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether Telegram may deliver the code as a flash call.
    #[must_use]
    pub fn allow_flash_call(mut self, allow_flash_call: bool) -> Self {
        self.allow_flash_call = allow_flash_call;
        self
    }

    /// Sets whether Telegram may deliver the code as a missed call.
    #[must_use]
    pub fn allow_missed_call(mut self, allow_missed_call: bool) -> Self {
        self.allow_missed_call = allow_missed_call;
        self
    }

    /// Sets whether the number is the one this device is already using.
    #[must_use]
    pub fn is_current_phone_number(mut self, is_current_phone_number: bool) -> Self {
        self.is_current_phone_number = is_current_phone_number;
        self
    }

    /// Sets whether the device has a SIM card whose number can't be checked.
    #[must_use]
    pub fn has_unknown_phone_number(mut self, has_unknown_phone_number: bool) -> Self {
        self.has_unknown_phone_number = has_unknown_phone_number;
        self
    }

    /// Sets whether Telegram may append an app hash to the SMS, so Android's SMS Retriever API can read the code.
    #[must_use]
    pub fn allow_sms_retriever_api(mut self, allow_sms_retriever_api: bool) -> Self {
        self.allow_sms_retriever_api = allow_sms_retriever_api;
        self
    }

    /// Lets Telegram deliver the code through Firebase on this device.
    #[must_use]
    pub fn firebase_authentication_settings(
        mut self,
        firebase_authentication_settings: FirebaseAuthenticationSettings,
    ) -> Self {
        self.firebase_authentication_settings = Some(firebase_authentication_settings);
        self
    }

    /// Passes the tokens (up to 20) received in previously logged out sessions.
    #[must_use]
    pub fn authentication_tokens(mut self, authentication_tokens: impl IntoIterator<Item = Vec<u8>>) -> Self {
        self.authentication_tokens = authentication_tokens.into_iter().collect();
        self
    }

    pub(crate) fn write(&self) -> base::CodeSettings {
        let (token, app_sandbox) = match &self.firebase_authentication_settings {
            Some(FirebaseAuthenticationSettings::Ios { device_token, is_app_sandbox }) => {
                (Some(device_token.clone()), Some(*is_app_sandbox))
            }
            _ => (None, None),
        };
        base::CodeSettings::CodeSettings(Box::new(types::CodeSettings {
            allow_flashcall: self.allow_flash_call,
            current_number: self.is_current_phone_number,
            allow_app_hash: self.allow_sms_retriever_api,
            allow_missed_call: self.allow_missed_call,
            allow_firebase: self.firebase_authentication_settings.is_some(),
            unknown_number: self.has_unknown_phone_number,
            logout_tokens: (!self.authentication_tokens.is_empty()).then(|| self.authentication_tokens.clone()),
            token,
            app_sandbox,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::{FirebaseAuthenticationSettings, PhoneNumberAuthenticationSettings};
    use crate::raw::base;

    fn written(settings: &PhoneNumberAuthenticationSettings) -> crate::raw::types::CodeSettings {
        let base::CodeSettings::CodeSettings(s) = settings.write();
        *s
    }

    #[test]
    fn debug_hides_tokens() {
        let settings = PhoneNumberAuthenticationSettings {
            firebase_authentication_settings: Some(FirebaseAuthenticationSettings::Ios {
                device_token: "secret-device".into(),
                is_app_sandbox: true,
            }),
            authentication_tokens: vec![b"secret-logout".to_vec()],
            ..Default::default()
        };
        let shown = format!("{settings:?}");
        assert!(!shown.contains("secret") && !shown.contains("115"), "{shown}");
        assert!(shown.contains("authentication_tokens: 1") && shown.contains("is_app_sandbox: true"), "{shown}");
    }

    #[test]
    fn settings_write_firebase_and_tokens() {
        let none = written(&PhoneNumberAuthenticationSettings::default());
        assert!(!none.allow_firebase && none.logout_tokens.is_none() && none.token.is_none());
        assert!(none.app_sandbox.is_none());

        let android = PhoneNumberAuthenticationSettings {
            firebase_authentication_settings: Some(FirebaseAuthenticationSettings::Android),
            ..Default::default()
        };
        let s = written(&android);
        assert!(s.allow_firebase && s.token.is_none() && s.app_sandbox.is_none());

        let ios = PhoneNumberAuthenticationSettings {
            allow_flash_call: true,
            is_current_phone_number: true,
            firebase_authentication_settings: Some(FirebaseAuthenticationSettings::Ios {
                device_token: "tok".into(),
                is_app_sandbox: true,
            }),
            authentication_tokens: vec![vec![1], vec![2, 3]],
            ..Default::default()
        };
        let s = written(&ios);
        assert!(s.allow_firebase && s.allow_flashcall && s.current_number);
        assert_eq!((s.token.as_deref(), s.app_sandbox), (Some("tok"), Some(true)));
        assert_eq!(s.logout_tokens, Some(vec![vec![1], vec![2, 3]]));
    }

    #[test]
    fn builds_from_new_and_setters() {
        let none = written(&PhoneNumberAuthenticationSettings::new());
        assert!(!none.allow_flashcall && !none.current_number && !none.allow_firebase && none.logout_tokens.is_none());
        let settings = PhoneNumberAuthenticationSettings::new()
            .allow_missed_call(true)
            .firebase_authentication_settings(FirebaseAuthenticationSettings::Android)
            .authentication_tokens([vec![7]]);
        let s = written(&settings);
        assert!(s.allow_missed_call && s.allow_firebase && !s.allow_flashcall && s.token.is_none());
        assert_eq!(s.logout_tokens, Some(vec![vec![7]]));
    }
}
