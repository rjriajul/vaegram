use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::StickerSet;
use crate::utils::input_sticker_set;

impl Client {
    pub async fn get_sticker_set(&self, name: &str) -> Result<StickerSet> {
        let request = functions::messages::GetStickerSet { stickerset: input_sticker_set(name), hash: 0 };
        StickerSet::parse(self.invoke(&request).await?)
    }
}
