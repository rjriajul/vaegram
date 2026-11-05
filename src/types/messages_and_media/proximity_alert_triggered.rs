use crate::types::User;

/// A user came within the alert distance of another user who watches for them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProximityAlertTriggered {
    /// The user who came close.
    pub traveler: User,
    /// The user who set the alert.
    pub watcher: User,
    /// In meters.
    pub distance: i32,
}
