use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::StickerSet;

const OLD_FEATURED_LIMIT: i32 = 100;

/// Request built by [`Client::get_featured_sticker_sets`].
pub struct GetFeaturedStickerSets<'a> {
    client: &'a Client,
    custom_emoji: bool,
    offset: Option<i32>,
    limit: Option<i32>,
}

impl Client {
    /// Lists the sticker sets Telegram currently features.
    ///
    /// Setting [`offset`](GetFeaturedStickerSets::offset) or [`limit`](GetFeaturedStickerSets::limit) lists the
    /// older featured sets instead.
    pub fn get_featured_sticker_sets(&self) -> GetFeaturedStickerSets<'_> {
        GetFeaturedStickerSets { client: self, custom_emoji: false, offset: None, limit: None }
    }
}

impl GetFeaturedStickerSets<'_> {
    /// Lists custom emoji sets instead of sticker sets.
    ///
    /// Fails with `Error::InvalidArgument` when combined with [`offset`](GetFeaturedStickerSets::offset) or
    /// [`limit`](GetFeaturedStickerSets::limit), because older featured sets are only listed for stickers.
    pub fn custom_emoji(mut self) -> Self {
        self.custom_emoji = true;
        self
    }

    /// Skips this many sets and pages through the older featured sets instead of the current ones.
    pub fn offset(mut self, offset: i32) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Caps how many older featured sets to get, 100 unless set. Like `offset`, it switches to the older sets.
    pub fn limit(mut self, limit: i32) -> Self {
        self.limit = Some(limit);
        self
    }
}

impl GetFeaturedStickerSets<'_> {
    async fn send(self) -> Result<Vec<StickerSet>> {
        let older = self.offset.is_some() || self.limit.is_some();
        let featured = match (self.custom_emoji, older) {
            (true, true) => {
                return Err(Error::InvalidArgument(
                    "older featured sets are only listed for stickers, not custom emoji",
                ));
            }
            (true, false) => self.client.invoke(&functions::messages::GetFeaturedEmojiStickers { hash: 0 }).await?,
            (false, true) => {
                let offset = self.offset.unwrap_or(0);
                let limit = self.limit.unwrap_or(OLD_FEATURED_LIMIT);
                self.client.invoke(&functions::messages::GetOldFeaturedStickers { offset, limit, hash: 0 }).await?
            }
            (false, false) => self.client.invoke(&functions::messages::GetFeaturedStickers { hash: 0 }).await?,
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
        client.get_featured_sticker_sets().await.unwrap();
        client.get_featured_sticker_sets().offset(20).limit(10).await.unwrap();
        client.get_featured_sticker_sets().limit(5).await.unwrap();
        let sent: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        let old = |offset, limit| GetOldFeaturedStickers { offset, limit, hash: 0 }.to_bytes();
        assert_eq!(sent, [GetFeaturedStickers { hash: 0 }.to_bytes(), old(20, 10), old(0, 5)]);
    }

    #[tokio::test]
    async fn custom_emoji_with_an_offset_is_refused() {
        let (client, mut rx) = Client::scripted(Vec::new());
        let refused = client.get_featured_sticker_sets().custom_emoji().offset(20).await;
        assert!(matches!(refused, Err(Error::InvalidArgument(_))), "{refused:?}");
        let refused = client.get_featured_sticker_sets().limit(5).custom_emoji().await;
        assert!(matches!(refused, Err(Error::InvalidArgument(_))), "{refused:?}");
        assert!(rx.try_recv().is_err());
    }
}
