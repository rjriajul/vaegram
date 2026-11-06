use super::get_archived_sticker_sets::SetPages;
use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::StickerSet;

/// Sticker sets the account created, built by [`Client::get_owned_sticker_sets`].
///
/// Awaiting it collects the sets into a `Vec`, up to [`limit`](GetOwnedStickerSets::limit);
/// [`next`](GetOwnedStickerSets::next) reads them one at a time. Sets are fetched 100 to a page.
#[must_use = "a pager does nothing until it is awaited or read with next()"]
pub struct GetOwnedStickerSets<'a> {
    client: &'a Client,
    pages: SetPages,
}

impl Client {
    /// Lists the sticker sets the account created.
    pub fn get_owned_sticker_sets(&self) -> GetOwnedStickerSets<'_> {
        GetOwnedStickerSets { client: self, pages: SetPages::new() }
    }
}

impl GetOwnedStickerSets<'_> {
    /// Starts after the set with this id instead of at the first set.
    pub fn offset_id(mut self, set_id: i64) -> Self {
        self.pages.sets.set_offset(set_id);
        self
    }

    /// Stops after this many sets in total. The default reads them all.
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.sets.remaining = limit;
        self
    }

    /// Returns the next set, or `None` once the sets or the limit run out.
    pub async fn next(&mut self) -> Result<Option<StickerSet>> {
        loop {
            if let Some(set) = self.pages.sets.pop() {
                return Ok(Some(set));
            }
            let Some(limit) = self.pages.sets.limit() else { return Ok(None) };
            let request =
                functions::messages::GetMyStickers { offset_id: self.pages.sets.offset().copied().unwrap_or(0), limit };
            let base::messages::MyStickers::MyStickers(page) = self.client.invoke(&request).await?;
            self.pages.push(&page.sets, page.count);
        }
    }
}

pager!(GetOwnedStickerSets, StickerSet);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::StickerType;
    use crate::errors::{Error, RpcError};
    use crate::raw::functions::messages::GetMyStickers;
    use crate::raw::{Serializable, base, types};

    fn reply(count: i32, sets: Vec<base::StickerSetCovered>) -> Vec<u8> {
        base::messages::MyStickers::MyStickers(Box::new(types::messages::MyStickers { count, sets })).to_bytes()
    }

    fn covered(id: i64) -> base::StickerSetCovered {
        let set = types::StickerSet {
            archived: false,
            official: false,
            masks: false,
            emojis: true,
            text_color: false,
            channel_emoji_status: false,
            creator: true,
            installed_date: None,
            id,
            access_hash: 12,
            title: "Mine".into(),
            short_name: "mine_by_bot".into(),
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

    #[tokio::test]
    async fn pages_through_the_sets_the_user_created() {
        let (client, mut rx) = Client::scripted(vec![Ok(reply(1, vec![covered(11)])), Ok(reply(1, Vec::new()))]);
        let sets = client.get_owned_sticker_sets().await.unwrap();
        assert_eq!(rx.try_recv().unwrap(), GetMyStickers { offset_id: 0, limit: 100 }.to_bytes());
        assert_eq!(sets.len(), 1);
        let set = &sets[0];
        assert_eq!((set.id, set.name.as_str(), set.title.as_str()), (11, "mine_by_bot", "Mine"));
        assert_eq!(set.sticker_type, StickerType::CustomEmoji);
        // Telegram counted one set and sent it, so nothing more is asked.
        assert!(rx.try_recv().is_err());

        // The account owns no more sets past this one.
        assert!(client.get_owned_sticker_sets().offset_id(77).limit(10).await.unwrap().is_empty());
        assert_eq!(rx.try_recv().unwrap(), GetMyStickers { offset_id: 77, limit: 10 }.to_bytes());
    }

    #[tokio::test]
    async fn the_next_page_starts_after_the_last_set() {
        let pages = vec![Ok(reply(5, vec![covered(11), covered(12)])), Ok(reply(5, vec![covered(13)]))];
        let (client, mut rx) = Client::scripted(pages);
        let mut sets = client.get_owned_sticker_sets().limit(3);
        let mut ids = Vec::new();
        while let Some(set) = sets.next().await.unwrap() {
            ids.push(set.id);
        }
        assert_eq!(ids, [11, 12, 13]);
        assert_eq!(rx.try_recv().unwrap(), GetMyStickers { offset_id: 0, limit: 3 }.to_bytes());
        assert_eq!(rx.try_recv().unwrap(), GetMyStickers { offset_id: 12, limit: 1 }.to_bytes());
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn telegram_errors_come_back_unchanged() {
        let (client, _rx) = Client::scripted(vec![Err(Error::Rpc(RpcError::parse(400, "OFFSET_INVALID")))]);
        let result = client.get_owned_sticker_sets().await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "OFFSET_INVALID"), "{result:?}");
    }
}
