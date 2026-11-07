use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{MaskPosition, Sticker, StickerSet};

impl Client {
    /// Changes where a mask sticker sits on a face and returns the updated set.
    ///
    /// [`Client::remove_sticker_mask_position`] clears the position.
    pub async fn set_sticker_mask_position(&self, sticker: &Sticker, position: MaskPosition) -> Result<StickerSet> {
        let request = functions::stickers::ChangeSticker {
            sticker: sticker.input_document(),
            emoji: None,
            mask_coords: Some(position.write()),
            keywords: None,
        };
        StickerSet::parse(self.invoke(&request).await?)
    }
}
