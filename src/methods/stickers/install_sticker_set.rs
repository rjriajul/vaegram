use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::StickerSet;
use crate::utils::input_sticker_set;

impl Client {
    /// Returns the sets Telegram archived to make room, usually none.
    pub async fn install_sticker_set(&self, name: &str) -> Result<Vec<StickerSet>> {
        let request = functions::messages::InstallStickerSet { stickerset: input_sticker_set(name), archived: false };
        Ok(match self.invoke(&request).await? {
            base::messages::StickerSetInstallResult::StickerSetInstallResultArchive(r) => {
                r.sets.iter().map(StickerSet::parse_covered).collect()
            }
            base::messages::StickerSetInstallResult::StickerSetInstallResultSuccess => Vec::new(),
        })
    }
}
