use crate::types::Chat;

/// A group or channel found near a location, with its distance in meters.
#[derive(Debug, Clone)]
pub struct NearbyChat {
    pub chat: Chat,
    pub distance: i32,
}
