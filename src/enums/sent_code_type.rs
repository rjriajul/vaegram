use std::fmt;

use crate::raw::base;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SentCodeType {
    App,
    Sms,
    Call,
    FlashCall,
    MissedCall,
    Email,
    FragmentSms,
    FirebaseSms,
    SmsWord,
    SmsPhrase,
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

    pub(crate) fn parse_next(kind: &base::auth::CodeType) -> Self {
        use base::auth::CodeType as T;
        match kind {
            T::CodeTypeSms => Self::Sms,
            T::CodeTypeCall => Self::Call,
            T::CodeTypeFlashCall => Self::FlashCall,
            T::CodeTypeMissedCall => Self::MissedCall,
            T::CodeTypeFragmentSms => Self::FragmentSms,
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

#[cfg(test)]
mod tests {
    use super::SentCodeType;
    use crate::raw::base::auth::CodeType;

    #[test]
    fn every_next_code_type_has_a_kind() {
        let kinds = [
            CodeType::CodeTypeSms,
            CodeType::CodeTypeCall,
            CodeType::CodeTypeFlashCall,
            CodeType::CodeTypeMissedCall,
            CodeType::CodeTypeFragmentSms,
        ]
        .map(|t| SentCodeType::parse_next(&t));
        let expected = [
            SentCodeType::Sms,
            SentCodeType::Call,
            SentCodeType::FlashCall,
            SentCodeType::MissedCall,
            SentCodeType::FragmentSms,
        ];
        assert_eq!(kinds, expected);
    }
}
