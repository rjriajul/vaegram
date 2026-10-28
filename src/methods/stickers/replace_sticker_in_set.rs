use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{InputSticker, PeerRef, Sticker, StickerSet};

impl Client {
    /// `user_id` is the set's owner, who new files are uploaded for.
    pub async fn replace_sticker_in_set(
        &self,
        user_id: impl Into<PeerRef>,
        old_sticker: &Sticker,
        sticker: InputSticker<'_>,
    ) -> Result<StickerSet> {
        let new_sticker = self.input_sticker(&user_id.into(), sticker).await?;
        let request = functions::stickers::ReplaceSticker { sticker: old_sticker.input_document(), new_sticker };
        StickerSet::parse(self.invoke(&request).await?)
    }
}
