use crate::Client;
use crate::methods::users::SetProfilePhoto;
use crate::types::Upload;

impl Client {
    pub fn set_business_account_profile_photo<'a>(
        &'a self,
        business_connection_id: &'a str,
        photo: impl Into<Upload<'a>>,
    ) -> SetProfilePhoto<'a> {
        SetProfilePhoto::new(self, Some(business_connection_id), photo.into())
    }
}
