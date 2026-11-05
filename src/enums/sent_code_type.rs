use std::fmt;

use crate::raw::base;

/// How Telegram delivered the login code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SentCodeType {
    /// A message in another Telegram app where the account is logged in.
    App,
    /// A text message.
    Sms,
    /// A phone call that reads the code out.
    Call,
    /// A flash call. The code is the last digits of the calling number.
    FlashCall,
    /// A missed call. The code is the last digits of the calling number.
    MissedCall,
    /// An email.
    Email,
    /// A text message sent through Fragment.
    FragmentSms,
    /// A text message sent through Firebase.
    FirebaseSms,
    /// A text message whose code is a word.
    SmsWord,
    /// A text message whose code is a phrase.
    SmsPhrase,
    /// Telegram asks for a login email, and the account has none yet. It has to be set up in an official app
    /// first.
    SetUpEmailRequired,
}

impl SentCodeType {
    pub(crate) fn parse(kind: &base::auth::SentCodeType) -> Self {
        use base::auth::SentCodeType as T;
        match kind {
            T::SentCodeTypeApp(_) => Self::App,
            T::SentCodeTypeSms(_) => Self::Sms,
            T::SentCodeTypeCall(_) => Self::Call,
            T::SentCodeTypeFlashCall(_) => Self::FlashCall,
            T::SentCodeTypeMissedCall(_) => Self::MissedCall,
            T::SentCodeTypeEmailCode(_) => Self::Email,
            T::SentCodeTypeSetUpEmailRequired(_) => Self::SetUpEmailRequired,
            T::SentCodeTypeFragmentSms(_) => Self::FragmentSms,
            T::SentCodeTypeFirebaseSms(_) => Self::FirebaseSms,
            T::SentCodeTypeSmsWord(_) => Self::SmsWord,
            T::SentCodeTypeSmsPhrase(_) => Self::SmsPhrase,
        }
    }
}

impl fmt::Display for SentCodeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::App => "Telegram app",
            Self::Sms | Self::FirebaseSms | Self::SmsWord | Self::SmsPhrase => "SMS",
            Self::Call => "phone call",
            Self::FlashCall => "phone flash call",
            Self::MissedCall => "missed call",
            Self::Email => "email",
            Self::FragmentSms => "Fragment SMS",
            Self::SetUpEmailRequired => "email (set one up in an official app first)",
        })
    }
}
