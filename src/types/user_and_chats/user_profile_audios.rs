use crate::types::Audio;

/// A page of the audios on a user's profile.
#[derive(Clone, Debug)]
pub struct UserProfileAudios {
    /// Number of audios on the profile, not only this page.
    pub total_count: i32,
    /// The audios on this page.
    pub audios: Vec<Audio>,
}
