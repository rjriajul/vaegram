use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::Sticker;

impl Client {
    /// Adds a sticker to the account's favorite stickers.
    pub async fn add_favorite_sticker(&self, sticker: &Sticker) -> Result<()> {
        self.invoke(&functions::messages::FaveSticker { id: sticker.input_document(), unfave: false }).await?;
        Ok(())
    }
}
