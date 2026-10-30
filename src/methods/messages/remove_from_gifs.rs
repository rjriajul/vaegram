use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::Animation;
use crate::types::messages_and_media::input_document;

impl Client {
    pub async fn remove_from_gifs(&self, gif: &Animation) -> Result<()> {
        self.invoke(&functions::messages::SaveGif { id: input_document(&gif.location), unsave: true }).await?;
        Ok(())
    }
}
