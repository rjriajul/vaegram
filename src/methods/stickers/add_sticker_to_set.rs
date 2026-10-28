use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{InputSticker, PeerRef, StickerSet};
use crate::utils::input_sticker_set;

impl Client {
    /// `user_id` is the set's owner, who new files are uploaded for.
    pub async fn add_sticker_to_set(
        &self,
        user_id: impl Into<PeerRef>,
        name: &str,
        sticker: InputSticker<'_>,
    ) -> Result<StickerSet> {
        let sticker = self.input_sticker(&user_id.into(), sticker).await?;
        let request = functions::stickers::AddStickerToSet { stickerset: input_sticker_set(name), sticker };
        StickerSet::parse(self.invoke(&request).await?)
    }
}
