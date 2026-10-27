use crate::types::Audio;

#[derive(Clone, Debug)]
pub struct UserProfileAudios {
    pub total_count: i32,
    pub audios: Vec<Audio>,
}
