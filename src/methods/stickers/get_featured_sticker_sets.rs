use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::StickerSet;

pub struct GetFeaturedStickerSets<'a> {
    client: &'a Client,
    custom_emoji: bool,
}

impl Client {
    pub fn get_featured_sticker_sets(&self) -> GetFeaturedStickerSets<'_> {
        GetFeaturedStickerSets { client: self, custom_emoji: false }
    }
}

impl GetFeaturedStickerSets<'_> {
    pub fn custom_emoji(mut self) -> Self {
        self.custom_emoji = true;
        self
    }
}

impl GetFeaturedStickerSets<'_> {
    async fn send(self) -> Result<Vec<StickerSet>> {
        let featured = if self.custom_emoji {
            self.client.invoke(&functions::messages::GetFeaturedEmojiStickers { hash: 0 }).await?
        } else {
            self.client.invoke(&functions::messages::GetFeaturedStickers { hash: 0 }).await?
        };
        Ok(match featured {
            base::messages::FeaturedStickers::FeaturedStickers(f) => {
                f.sets.iter().map(StickerSet::parse_covered).collect()
            }
            base::messages::FeaturedStickers::FeaturedStickersNotModified(_) => Vec::new(),
        })
    }
}

into_future!(GetFeaturedStickerSets, Vec<StickerSet>);
