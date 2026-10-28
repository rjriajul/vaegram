use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::StickerSet;
use crate::utils::input_sticker_set;

impl Client {
    /// Shows one of the set's custom emoji as its thumbnail, or the first one with `None`.
    pub async fn set_custom_emoji_sticker_set_thumbnail(
        &self,
        name: &str,
        custom_emoji_id: Option<i64>,
    ) -> Result<StickerSet> {
        let request = functions::stickers::SetStickerSetThumb {
            stickerset: input_sticker_set(name),
            thumb: None,
            // Zero is how Telegram is told to fall back to the first emoji.
            thumb_document_id: Some(custom_emoji_id.unwrap_or(0)),
        };
        StickerSet::parse(self.invoke(&request).await?)
    }
}
