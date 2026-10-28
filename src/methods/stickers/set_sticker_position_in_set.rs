use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{Sticker, StickerSet};

impl Client {
    /// Moves a sticker to a zero-based position in its set.
    pub async fn set_sticker_position_in_set(&self, sticker: &Sticker, position: i32) -> Result<StickerSet> {
        let request = functions::stickers::ChangeStickerPosition { sticker: sticker.input_document(), position };
        StickerSet::parse(self.invoke(&request).await?)
    }
}
