use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::utils::input_sticker_set;

impl Client {
    pub async fn delete_sticker_set(&self, name: &str) -> Result<()> {
        self.invoke(&functions::stickers::DeleteStickerSet { stickerset: input_sticker_set(name) }).await?;
        Ok(())
    }
}
