use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::utils::input_sticker_set;

impl Client {
    /// Deletes the sticker set with this short `name` for everyone.
    pub async fn delete_sticker_set(&self, name: &str) -> Result<()> {
        self.invoke(&functions::stickers::DeleteStickerSet { stickerset: input_sticker_set(name) }).await?;
        Ok(())
    }
}
