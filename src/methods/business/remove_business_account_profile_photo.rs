use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};

impl Client {
    /// Removes the profile photo of the business account behind a connection.
    /// With `is_public` it removes the public photo instead of the main one.
    pub async fn remove_business_account_profile_photo(
        &self,
        business_connection_id: &str,
        is_public: bool,
    ) -> Result<()> {
        let id = base::InputPhoto::InputPhotoEmpty;
        let request = functions::photos::UpdateProfilePhoto { fallback: is_public, bot: None, id };
        self.invoke_business(Some(business_connection_id), request).await?;
        Ok(())
    }
}
