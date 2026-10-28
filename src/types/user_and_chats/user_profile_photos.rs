use crate::types::Photo;

#[derive(Debug, Clone)]
pub struct UserProfilePhotos {
    pub total_count: i32,
    pub photos: Vec<Photo>,
}
