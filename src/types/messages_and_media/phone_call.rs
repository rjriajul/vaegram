use crate::enums::PhoneCallDiscardReason;

/// A private call that ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhoneCallEnded {
    pub id: i64,
    pub is_video: bool,
    pub reason: PhoneCallDiscardReason,
    /// In seconds; `None` when the call never connected.
    pub duration: Option<i32>,
}
