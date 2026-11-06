use crate::Client;
use crate::errors::{Error, Result};
use crate::methods::pages::OffsetPages;
use crate::raw::{base, functions};
use crate::types::StickerSet;

/// Featured sticker sets, built by [`Client::get_featured_sticker_sets`].
///
/// Awaiting it collects the sets into a `Vec`, up to [`limit`](GetFeaturedStickerSets::limit);
/// [`next`](GetFeaturedStickerSets::next) reads them one at a time. The current featured sets come in one request;
/// the older ones, listed after [`offset`](GetFeaturedStickerSets::offset), 100 to a page.
#[must_use = "a pager does nothing until it is awaited or read with next()"]
pub struct GetFeaturedStickerSets<'a> {
    client: &'a Client,
    custom_emoji: bool,
    older: bool,
    pages: OffsetPages<StickerSet, i32>,
}

impl Client {
    /// Lists the sticker sets Telegram currently features.
    ///
    /// Setting [`offset`](GetFeaturedStickerSets::offset) lists the older featured sets instead.
    pub fn get_featured_sticker_sets(&self) -> GetFeaturedStickerSets<'_> {
        let pages = OffsetPages::starting_at(100, 0);
        GetFeaturedStickerSets { client: self, custom_emoji: false, older: false, pages }
    }
}

impl GetFeaturedStickerSets<'_> {
    /// Lists custom emoji sets instead of sticker sets.
    ///
    /// Reading fails with `Error::InvalidArgument` when combined with
    /// [`offset`](GetFeaturedStickerSets::offset), because older featured sets are only listed for stickers.
    pub fn custom_emoji(mut self) -> Self {
        self.custom_emoji = true;
        self
    }

    /// Lists the older featured sets instead of the current ones, skipping this many. `0` starts at the newest of
    /// them.
    pub fn offset(mut self, offset: i32) -> Self {
        self.older = true;
        self.pages.set_offset(offset);
        self
    }

    /// Stops after this many sets in total. The default reads them all.
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    /// Returns the next set, or `None` once the sets or the limit run out.
    pub async fn next(&mut self) -> Result<Option<StickerSet>> {
        // Checked before the limit, so a zero limit doesn't hide the mistake.
        if self.custom_emoji && self.older {
            return Err(Error::InvalidArgument("older featured sets are only listed for stickers, not custom emoji"));
        }
        loop {
            if let Some(set) = self.pages.pop() {
                return Ok(Some(set));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let client = self.client;
            let featured = if self.older {
                let offset = self.pages.offset().copied().unwrap_or(0);
                client.invoke(&functions::messages::GetOldFeaturedStickers { offset, limit, hash: 0 }).await?
            } else if self.custom_emoji {
                client.invoke(&functions::messages::GetFeaturedEmojiStickers { hash: 0 }).await?
            } else {
                client.invoke(&functions::messages::GetFeaturedStickers { hash: 0 }).await?
            };
            match featured {
                base::messages::FeaturedStickers::FeaturedStickers(f) => {
                    let sets = f.sets.iter().map(StickerSet::parse_covered).collect();
                    if self.older {
                        self.pages.push_counted(sets, f.sets.len(), f.count);
                    } else {
                        // The current featured sets come whole.
                        self.pages.push(sets, None);
                    }
                }
                base::messages::FeaturedStickers::FeaturedStickersNotModified(_) => self.pages.push(Vec::new(), None),
            }
        }
    }
}

pager!(GetFeaturedStickerSets, StickerSet);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::functions::messages::{GetFeaturedStickers, GetOldFeaturedStickers};
    use crate::raw::{Serializable, base, types};

    fn none() -> crate::Result<Vec<u8>> {
        let none = types::messages::FeaturedStickersNotModified { count: 0 };
        Ok(base::messages::FeaturedStickers::FeaturedStickersNotModified(none).to_bytes())
    }

    #[tokio::test]
    async fn an_offset_asks_for_older_sets() {
        let (client, mut rx) = Client::scripted(vec![none(), none(), none()]);
        client.get_featured_sticker_sets().limit(5).await.unwrap();
        client.get_featured_sticker_sets().offset(20).limit(10).await.unwrap();
        client.get_featured_sticker_sets().offset(0).await.unwrap();
        let sent: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        let old = |offset, limit| GetOldFeaturedStickers { offset, limit, hash: 0 }.to_bytes();
        assert_eq!(sent, [GetFeaturedStickers { hash: 0 }.to_bytes(), old(20, 10), old(0, 100)]);
    }

    #[tokio::test]
    async fn custom_emoji_with_an_offset_is_refused() {
        let (client, mut rx) = Client::scripted(Vec::new());
        let refused = client.get_featured_sticker_sets().custom_emoji().offset(20).await;
        assert!(matches!(refused, Err(Error::InvalidArgument(_))), "{refused:?}");
        let refused = client.get_featured_sticker_sets().offset(0).custom_emoji().await;
        assert!(matches!(refused, Err(Error::InvalidArgument(_))), "{refused:?}");
        let refused = client.get_featured_sticker_sets().offset(0).custom_emoji().limit(0).await;
        assert!(matches!(refused, Err(Error::InvalidArgument(_))), "{refused:?}");
        assert!(rx.try_recv().is_err());
    }

    fn covered(id: i64) -> base::StickerSetCovered {
        let set = types::StickerSet {
            archived: false,
            official: false,
            masks: false,
            emojis: false,
            text_color: false,
            channel_emoji_status: false,
            creator: false,
            installed_date: None,
            id,
            access_hash: 12,
            title: "Featured".into(),
            short_name: "featured".into(),
            thumbs: None,
            thumb_dc_id: None,
            thumb_version: None,
            thumb_document_id: None,
            count: 1,
            hash: 0,
        };
        let cover = base::Document::DocumentEmpty(types::DocumentEmpty { id: 1 });
        let covered = types::StickerSetCovered { set: base::StickerSet::StickerSet(Box::new(set)), cover };
        base::StickerSetCovered::StickerSetCovered(Box::new(covered))
    }

    fn featured(count: i32, ids: &[i64]) -> crate::Result<Vec<u8>> {
        let sets = ids.iter().map(|&id| covered(id)).collect();
        let page = types::messages::FeaturedStickers { premium: false, hash: 0, count, sets, unread: Vec::new() };
        Ok(base::messages::FeaturedStickers::FeaturedStickers(Box::new(page)).to_bytes())
    }

    #[tokio::test]
    async fn older_sets_are_read_page_by_page_up_to_the_count() {
        let (client, mut rx) = Client::scripted(vec![featured(3, &[11, 12]), featured(3, &[13])]);
        let sets = client.get_featured_sticker_sets().offset(0).await.unwrap();
        assert_eq!(sets.iter().map(|s| s.id).collect::<Vec<_>>(), [11, 12, 13]);
        let sent: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        let old = |offset, limit| GetOldFeaturedStickers { offset, limit, hash: 0 }.to_bytes();
        assert_eq!(sent, [old(0, 100), old(2, 100)]);
    }
}
