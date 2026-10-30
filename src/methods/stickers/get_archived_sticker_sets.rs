use crate::Client;
use crate::enums::StickerType;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::StickerSet;

pub struct GetArchivedStickerSets<'a> {
    client: &'a Client,
    request: functions::messages::GetArchivedStickers,
}

impl Client {
    pub fn get_archived_sticker_sets(&self, sticker_type: StickerType) -> GetArchivedStickerSets<'_> {
        let request = functions::messages::GetArchivedStickers {
            masks: sticker_type == StickerType::Mask,
            emojis: sticker_type == StickerType::CustomEmoji,
            offset_id: 0,
            limit: 100,
        };
        GetArchivedStickerSets { client: self, request }
    }
}

impl GetArchivedStickerSets<'_> {
    /// Continues after the set with this id, the last one of the previous page.
    pub fn offset_id(mut self, set_id: i64) -> Self {
        self.request.offset_id = set_id;
        self
    }

    pub fn limit(mut self, limit: i32) -> Self {
        self.request.limit = limit;
        self
    }
}

impl GetArchivedStickerSets<'_> {
    async fn send(self) -> Result<Vec<StickerSet>> {
        let base::messages::ArchivedStickers::ArchivedStickers(page) = self.client.invoke(&self.request).await?;
        Ok(page.sets.iter().map(StickerSet::parse_covered).collect())
    }
}

into_future!(GetArchivedStickerSets, Vec<StickerSet>);
