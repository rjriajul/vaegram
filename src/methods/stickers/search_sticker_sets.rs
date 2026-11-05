use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::StickerSet;

/// Request built by [`Client::search_sticker_sets`].
pub struct SearchStickerSets<'a> {
    client: &'a Client,
    query: String,
    custom_emoji: bool,
    exclude_featured: bool,
}

impl Client {
    /// Searches sticker sets by title or name.
    pub fn search_sticker_sets(&self, query: impl Into<String>) -> SearchStickerSets<'_> {
        SearchStickerSets { client: self, query: query.into(), custom_emoji: false, exclude_featured: false }
    }
}

impl SearchStickerSets<'_> {
    /// Searches custom emoji sets instead of sticker sets.
    pub fn custom_emoji(mut self) -> Self {
        self.custom_emoji = true;
        self
    }

    /// Leaves out featured sets.
    pub fn exclude_featured(mut self) -> Self {
        self.exclude_featured = true;
        self
    }
}

impl SearchStickerSets<'_> {
    async fn send(self) -> Result<Vec<StickerSet>> {
        let (q, exclude_featured) = (self.query, self.exclude_featured);
        let found = if self.custom_emoji {
            self.client.invoke(&functions::messages::SearchEmojiStickerSets { exclude_featured, q, hash: 0 }).await?
        } else {
            self.client.invoke(&functions::messages::SearchStickerSets { exclude_featured, q, hash: 0 }).await?
        };
        Ok(match found {
            base::messages::FoundStickerSets::FoundStickerSets(found) => {
                found.sets.iter().map(StickerSet::parse_covered).collect()
            }
            base::messages::FoundStickerSets::FoundStickerSetsNotModified => Vec::new(),
        })
    }
}

into_future!(SearchStickerSets, Vec<StickerSet>);
