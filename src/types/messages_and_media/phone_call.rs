use crate::enums::PhoneCallDiscardReason;

/// A private call that ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhoneCallEnded {
    /// Id of the call.
    pub id: i64,
    /// The call was a video call.
    pub is_video: bool,
    /// Why the call ended.
    pub reason: PhoneCallDiscardReason,
    /// In seconds; `None` when the call never connected.
    pub duration: Option<i32>,
}
