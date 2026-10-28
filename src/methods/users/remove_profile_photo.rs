use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};

impl Client {
    pub async fn remove_profile_photo(&self) -> Result<()> {
        let id = base::InputPhoto::InputPhotoEmpty;
        self.invoke(&functions::photos::UpdateProfilePhoto { fallback: false, bot: None, id }).await?;
        Ok(())
    }
}
