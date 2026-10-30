use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::Sticker;

impl Client {
    pub async fn get_favorite_stickers(&self) -> Result<Vec<Sticker>> {
        Ok(match self.invoke(&functions::messages::GetFavedStickers { hash: 0 }).await? {
            base::messages::FavedStickers::FavedStickers(f) => f.stickers.iter().filter_map(Sticker::parse).collect(),
            base::messages::FavedStickers::FavedStickersNotModified => Vec::new(),
        })
    }
}
