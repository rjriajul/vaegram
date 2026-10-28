use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::StickerSet;
use crate::utils::input_sticker_set;

impl Client {
    pub async fn set_sticker_set_title(&self, name: &str, title: impl Into<String>) -> Result<StickerSet> {
        let (stickerset, title) = (input_sticker_set(name), title.into());
        let request = functions::stickers::RenameStickerSet { stickerset, title };
        StickerSet::parse(self.invoke(&request).await?)
    }
}
