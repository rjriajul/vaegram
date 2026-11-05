use crate::types::Chat;

/// A group or channel found near a location, with its distance in meters.
#[derive(Debug, Clone)]
pub struct NearbyChat {
    /// The group or channel.
    pub chat: Chat,
    /// Distance from the searched location, in meters.
    pub distance: i32,
}
