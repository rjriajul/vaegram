use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::messages_and_media::input_document;
use crate::types::{PeerRef, StickerSet, Upload};
use crate::utils::input_sticker_set;

impl Client {
    /// Sets the thumbnail of a regular or mask set, or removes it with `None` so apps show the first sticker.
    ///
    /// `user_id` is the set's owner, who the file is uploaded for.
    pub async fn set_sticker_set_thumbnail(
        &self,
        name: &str,
        user_id: impl Into<PeerRef>,
        thumbnail: Option<Upload<'_>>,
    ) -> Result<StickerSet> {
        let user: PeerRef = user_id.into();
        #[cfg(feature = "bot-api")]
        if let Some(api) = self.0.bot_api.as_deref() {
            let user_id = api.peer_id(self, &user).await?;
            return StickerSet::parse(api.set_sticker_set_thumbnail(name, user_id, thumbnail).await?);
        }
        let thumb = match thumbnail {
            Some(file) => Some(input_document(&self.upload_sticker(&user, file).await?.location)),
            None => None,
        };
        let stickerset = input_sticker_set(name);
        let request = functions::stickers::SetStickerSetThumb { stickerset, thumb, thumb_document_id: None };
        StickerSet::parse(self.invoke(&request).await?)
    }
}
