use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{InputSticker, PeerRef, Sticker, StickerSet};

impl Client {
    /// `user_id` is the set's owner, who new files are uploaded for.
    ///
    /// In Bot API mode (`ClientBuilder::bot_api`) `old_sticker` must name its set, as one from
    /// [`get_sticker_set`](Client::get_sticker_set) does, and a sticker given by
    /// [`InputSticker::existing`](crate::types::InputSticker::existing) goes as a static one.
    pub async fn replace_sticker_in_set(
        &self,
        user_id: impl Into<PeerRef>,
        old_sticker: &Sticker,
        sticker: InputSticker<'_>,
    ) -> Result<StickerSet> {
        let user: PeerRef = user_id.into();
        // A new file goes in the `replaceStickerInSet` request itself; the old sticker goes by its file id.
        #[cfg(feature = "bot-api")]
        if let Some(api) = self.0.bot_api.as_deref() {
            let user_id = api.peer_id(self, &user).await?;
            return StickerSet::parse(api.replace_sticker(user_id, old_sticker, sticker).await?);
        }
        let new_sticker = self.input_sticker(&user, sticker).await?;
        let request = functions::stickers::ReplaceSticker { sticker: old_sticker.input_document(), new_sticker };
        StickerSet::parse(self.invoke(&request).await?)
    }
}
