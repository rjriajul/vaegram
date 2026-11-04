use crate::Client;
use crate::types::{PeerRef, Upload};

use super::SetProfilePhoto;

impl Client {
    /// Sets the profile photo of a bot the account owns.
    pub fn set_bot_profile_photo<'a>(
        &'a self,
        bot_id: impl Into<PeerRef>,
        photo: impl Into<Upload<'a>>,
    ) -> SetProfilePhoto<'a> {
        SetProfilePhoto::new(self, None, photo.into()).bot(bot_id.into())
    }
}
