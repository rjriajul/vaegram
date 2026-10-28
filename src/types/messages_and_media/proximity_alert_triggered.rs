use crate::types::User;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProximityAlertTriggered {
    pub traveler: User,
    pub watcher: User,
    /// In meters.
    pub distance: i32,
}
