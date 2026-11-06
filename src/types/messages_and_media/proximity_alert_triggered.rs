use crate::types::User;

/// Service message sent when a user comes within the alert distance set by another user.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ProximityAlertTriggered {
    /// The user who came close.
    pub traveler: User,
    /// The user who set the alert.
    pub watcher: User,
    /// Distance between the two users, in meters.
    pub distance: i32,
}
