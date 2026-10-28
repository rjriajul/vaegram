use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::Photo;
use crate::types::messages_and_media::input_photo;

impl Client {
    pub async fn delete_profile_photos(&self, photos: &[Photo]) -> Result<()> {
        let id = photos.iter().map(|photo| input_photo(&photo.location)).collect();
        self.invoke(&functions::photos::DeletePhotos { id }).await?;
        Ok(())
    }
}
