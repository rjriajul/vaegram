use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{Sticker, StickerSet};

impl Client {
    /// `emoji` is written as in [`InputSticker::new`](crate::types::InputSticker::new).
    ///
    /// In Bot API mode (`ClientBuilder::bot_api`) `sticker` must name its set, as one from
    /// [`get_sticker_set`](Client::get_sticker_set) does: the Bot API reads the set back by its name.
    pub async fn set_sticker_emoji_list(&self, sticker: &Sticker, emoji: impl Into<String>) -> Result<StickerSet> {
        let emoji: String = emoji.into();
        // The Bot API names the sticker by its file id, and its set by the set's name.
        #[cfg(feature = "bot-api")]
        if let Some(api) = self.0.bot_api.as_deref() {
            return StickerSet::parse(api.sticker_emoji(sticker, &emoji).await?);
        }
        let request = functions::stickers::ChangeSticker {
            sticker: sticker.input_document(),
            emoji: Some(emoji),
            mask_coords: None,
            keywords: None,
        };
        StickerSet::parse(self.invoke(&request).await?)
    }
}
