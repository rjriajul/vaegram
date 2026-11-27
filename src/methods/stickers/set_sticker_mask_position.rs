use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{MaskPosition, Sticker, StickerSet};

impl Client {
    /// Changes where a mask sticker sits on a face and returns the updated set.
    ///
    /// [`Client::remove_sticker_mask_position`] clears the position.
    ///
    /// In Bot API mode (`ClientBuilder::bot_api`) `sticker` must name its set, as one from
    /// [`get_sticker_set`](Client::get_sticker_set) does: the Bot API reads the set back by its name.
    pub async fn set_sticker_mask_position(&self, sticker: &Sticker, position: MaskPosition) -> Result<StickerSet> {
        // The Bot API names the sticker by its file id, and its set by the set's name.
        #[cfg(feature = "bot-api")]
        if let Some(api) = self.0.bot_api.as_deref() {
            return StickerSet::parse(api.sticker_mask(sticker, Some(&position)).await?);
        }
        let request = functions::stickers::ChangeSticker {
            sticker: sticker.input_document(),
            emoji: None,
            mask_coords: Some(position.write()),
            keywords: None,
        };
        StickerSet::parse(self.invoke(&request).await?)
    }
}
