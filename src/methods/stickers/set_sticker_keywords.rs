use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{InputSticker, Sticker, StickerSet};

impl Client {
    /// Replaces a sticker's search keywords; an empty list removes them.
    pub async fn set_sticker_keywords(
        &self,
        sticker: &Sticker,
        keywords: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<StickerSet> {
        let request = functions::stickers::ChangeSticker {
            sticker: sticker.input_document(),
            emoji: None,
            mask_coords: None,
            keywords: Some(InputSticker::join_keywords(keywords)),
        };
        StickerSet::parse(self.invoke(&request).await?)
    }
}
