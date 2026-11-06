use crate::enums::{NextCodeType, SentCodeType};

/// A login code Telegram has sent.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SentCode {
    /// How the code was delivered.
    pub kind: SentCodeType,
    /// Hash that identifies this code request. Pass it back when signing in or resending the code.
    pub phone_code_hash: String,
    /// How a resend would deliver the code. `None` if Telegram offers no other way.
    pub next_kind: Option<NextCodeType>,
    /// Seconds to wait before resending the code. `None` if Telegram sent no timeout.
    pub timeout: Option<i32>,
}
