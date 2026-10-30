use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::Sticker;

impl Client {
    pub async fn get_recent_stickers(&self) -> Result<Vec<Sticker>> {
        let recent = self.invoke(&functions::messages::GetRecentStickers { attached: false, hash: 0 }).await?;
        Ok(match recent {
            base::messages::RecentStickers::RecentStickers(r) => r.stickers.iter().filter_map(Sticker::parse).collect(),
            base::messages::RecentStickers::RecentStickersNotModified => Vec::new(),
        })
    }
}
