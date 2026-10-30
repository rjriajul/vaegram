use crate::Client;
use crate::enums::StickerType;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::StickerSet;

impl Client {
    pub async fn get_installed_sticker_sets(&self, sticker_type: StickerType) -> Result<Vec<StickerSet>> {
        let all = match sticker_type {
            StickerType::Regular => self.invoke(&functions::messages::GetAllStickers { hash: 0 }).await?,
            StickerType::Mask => self.invoke(&functions::messages::GetMaskStickers { hash: 0 }).await?,
            StickerType::CustomEmoji => self.invoke(&functions::messages::GetEmojiStickers { hash: 0 }).await?,
        };
        Ok(match all {
            base::messages::AllStickers::AllStickers(all) => {
                all.sets.iter().map(|set| StickerSet::from_raw(set, &[])).collect()
            }
            base::messages::AllStickers::AllStickersNotModified => Vec::new(),
        })
    }
}
