use std::fmt;

use crate::raw::base;

/// How Telegram will deliver the code if it is asked to send it again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NextCodeType {
    /// A text message.
    Sms,
    /// A phone call that reads the code out.
    Call,
    /// A flash call. The code is the last digits of the calling number.
    FlashCall,
    /// A missed call. The code is the last digits of the calling number.
    MissedCall,
    /// A text message sent through Fragment.
    FragmentSms,
}

impl NextCodeType {
    pub(crate) fn parse(kind: &base::auth::CodeType) -> Self {
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

impl fmt::Display for NextCodeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Sms => "SMS",
            Self::Call => "phone call",
            Self::FlashCall => "phone flash call",
            Self::MissedCall => "missed call",
            Self::FragmentSms => "Fragment SMS",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::NextCodeType;
    use crate::raw::base::auth::CodeType;

    #[test]
    fn every_next_code_type_maps() {
        let kinds = [
            CodeType::CodeTypeSms,
            CodeType::CodeTypeCall,
            CodeType::CodeTypeFlashCall,
            CodeType::CodeTypeMissedCall,
            CodeType::CodeTypeFragmentSms,
        ]
        .map(|t| NextCodeType::parse(&t));
        let expected = [
            NextCodeType::Sms,
            NextCodeType::Call,
            NextCodeType::FlashCall,
            NextCodeType::MissedCall,
            NextCodeType::FragmentSms,
        ];
        assert_eq!(kinds, expected);
        assert_eq!(NextCodeType::FlashCall.to_string(), "phone flash call");
    }
}
