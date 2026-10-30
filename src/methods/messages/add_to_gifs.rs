use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::Animation;
use crate::types::messages_and_media::input_document;

impl Client {
    pub async fn add_to_gifs(&self, gif: &Animation) -> Result<()> {
        self.invoke(&functions::messages::SaveGif { id: input_document(&gif.location), unsave: false }).await?;
        Ok(())
    }
}
