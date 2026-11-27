use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{InputSticker, Sticker, StickerSet};

impl Client {
    /// Replaces a sticker's search keywords; an empty list removes them.
    ///
    /// In Bot API mode (`ClientBuilder::bot_api`) `sticker` must name its set, as one from
    /// [`get_sticker_set`](Client::get_sticker_set) does: the Bot API reads the set back by its name.
    pub async fn set_sticker_keywords(
        &self,
        sticker: &Sticker,
        keywords: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<StickerSet> {
        let keywords = InputSticker::join_keywords(keywords);
        // The Bot API names the sticker by its file id, and its set by the set's name.
        #[cfg(feature = "bot-api")]
        if let Some(api) = self.0.bot_api.as_deref() {
            return StickerSet::parse(api.sticker_keywords(sticker, &keywords).await?);
        }
        let request = functions::stickers::ChangeSticker {
            sticker: sticker.input_document(),
            emoji: None,
            mask_coords: None,
            keywords: Some(keywords),
        };
        StickerSet::parse(self.invoke(&request).await?)
    }
}
