use crate::types::Photo;

/// A page of a user's profile photos.
#[derive(Debug, Clone)]
pub struct UserProfilePhotos {
    /// Number of profile photos, not only this page.
    pub total_count: i32,
    /// The photos on this page.
    pub photos: Vec<Photo>,
}
