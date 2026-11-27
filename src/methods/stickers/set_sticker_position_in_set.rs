use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{Sticker, StickerSet};

impl Client {
    /// Moves a sticker to a zero-based position in its set.
    ///
    /// In Bot API mode (`ClientBuilder::bot_api`) `sticker` must name its set, as one from
    /// [`get_sticker_set`](Client::get_sticker_set) does: the Bot API reads the set back by its name.
    pub async fn set_sticker_position_in_set(&self, sticker: &Sticker, position: i32) -> Result<StickerSet> {
        // The Bot API names the sticker by its file id, and its set by the set's name.
        #[cfg(feature = "bot-api")]
        if let Some(api) = self.0.bot_api.as_deref() {
            return StickerSet::parse(api.sticker_position(sticker, position).await?);
        }
        let request = functions::stickers::ChangeStickerPosition { sticker: sticker.input_document(), position };
        StickerSet::parse(self.invoke(&request).await?)
    }
}
