use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{Sticker, StickerSet};

impl Client {
    /// Removes a sticker from its set and returns the updated [`StickerSet`].
    ///
    /// In Bot API mode (`ClientBuilder::bot_api`) `sticker` must name its set, as one from
    /// [`get_sticker_set`](Client::get_sticker_set) does: the Bot API reads the set back by its name.
    pub async fn delete_sticker_from_set(&self, sticker: &Sticker) -> Result<StickerSet> {
        // The Bot API names the sticker by its file id, and its set by the set's name.
        #[cfg(feature = "bot-api")]
        if let Some(api) = self.0.bot_api.as_deref() {
            return StickerSet::parse(api.delete_sticker(sticker).await?);
        }
        let request = functions::stickers::RemoveStickerFromSet { sticker: sticker.input_document() };
        StickerSet::parse(self.invoke(&request).await?)
    }
}
