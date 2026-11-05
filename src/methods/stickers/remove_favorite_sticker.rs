use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::Sticker;

impl Client {
    /// Removes a sticker from the account's favorite stickers.
    pub async fn remove_favorite_sticker(&self, sticker: &Sticker) -> Result<()> {
        self.invoke(&functions::messages::FaveSticker { id: sticker.input_document(), unfave: true }).await?;
        Ok(())
    }
}
