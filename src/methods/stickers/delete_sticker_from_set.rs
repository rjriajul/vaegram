use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{Sticker, StickerSet};

impl Client {
    pub async fn delete_sticker_from_set(&self, sticker: &Sticker) -> Result<StickerSet> {
        let request = functions::stickers::RemoveStickerFromSet { sticker: sticker.input_document() };
        StickerSet::parse(self.invoke(&request).await?)
    }
}
