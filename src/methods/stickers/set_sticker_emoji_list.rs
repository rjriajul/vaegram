use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{Sticker, StickerSet};

impl Client {
    /// `emoji` is written as in [`InputSticker::new`](crate::types::InputSticker::new).
    pub async fn set_sticker_emoji_list(&self, sticker: &Sticker, emoji: impl Into<String>) -> Result<StickerSet> {
        let request = functions::stickers::ChangeSticker {
            sticker: sticker.input_document(),
            emoji: Some(emoji.into()),
            mask_coords: None,
            keywords: None,
        };
        StickerSet::parse(self.invoke(&request).await?)
    }
}
