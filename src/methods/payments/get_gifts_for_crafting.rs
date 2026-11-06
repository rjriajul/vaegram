use super::get_chat_gifts::push_gifts;
use crate::Client;
use crate::errors::Result;
use crate::methods::pages::OffsetPages;
use crate::raw::functions;
use crate::types::{OwnedGift, PeerRef};

/// Gifts that can go into crafting, built by [`Client::get_gifts_for_crafting`].
///
/// Awaiting it collects the gifts into a `Vec`, up to [`limit`](GetGiftsForCrafting::limit);
/// [`next`](GetGiftsForCrafting::next) reads them one at a time. Gifts are fetched 100 to a page.
#[must_use = "a pager does nothing until it is awaited or read with next()"]
pub struct GetGiftsForCrafting<'a> {
    client: &'a Client,
    gift_id: i64,
    owner_id: Option<i64>,
    total: Option<i32>,
    pages: OffsetPages<OwnedGift>,
}

impl Client {
    /// Lists the user's gifts that can go into crafting an upgraded `gift_id` gift.
    pub fn get_gifts_for_crafting(&self, gift_id: i64) -> GetGiftsForCrafting<'_> {
        GetGiftsForCrafting { client: self, gift_id, owner_id: None, total: None, pages: OffsetPages::new(100) }
    }
}

impl GetGiftsForCrafting<'_> {
    /// Starts at this offset token, taken from [`next_offset`](Self::next_offset) of an earlier pager, instead of
    /// the first gift.
    pub fn offset(mut self, offset: impl Into<String>) -> Self {
        self.pages.set_offset(offset.into());
        self
    }

    /// Where the unread part of the list starts, for `offset` on a later pager; `None` once the list has ended.
    /// Gifts already fetched but not yet returned by `next` come before it. Before the first page it is the
    /// starting point: `Some("")`, meaning from the start, unless `offset` set another.
    pub fn next_offset(&self) -> Option<&str> {
        self.pages.offset().map(String::as_str)
    }

    /// Stops after this many gifts in total. The default reads the whole list.
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    /// Returns how many gifts can go into crafting in all, ignoring `limit` and `offset`.
    ///
    /// Before the first page this reads one, which `next` then returns from without another request.
    pub async fn total(&mut self) -> Result<i32> {
        if self.total.is_none() {
            self.fetch(self.pages.limit().unwrap_or(1)).await?;
        }
        Ok(self.total.unwrap_or(0))
    }

    /// Returns the next gift, or `None` once the list or the limit is exhausted.
    pub async fn next(&mut self) -> Result<Option<OwnedGift>> {
        loop {
            if let Some(gift) = self.pages.pop() {
                return Ok(Some(gift));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            self.fetch(limit).await?;
        }
    }

    async fn fetch(&mut self, limit: i32) -> Result<()> {
        let client = self.client;
        let owner_id = match self.owner_id {
            Some(id) => id,
            None => client.resolve(&PeerRef::Me).await?.0,
        };
        self.owner_id = Some(owner_id);
        let offset = self.pages.next_offset().unwrap_or_default().into();
        let request = functions::payments::GetCraftStarGifts { gift_id: self.gift_id, offset, limit };
        let page = client.invoke(&request).await?;
        self.total = Some(push_gifts(client, &mut self.pages, page, owner_id));
        Ok(())
    }
}

pager!(GetGiftsForCrafting, OwnedGift);

#[cfg(test)]
mod tests {
    use super::super::get_chat_gifts::tests::{empty_page, ids, page};
    use crate::Client;
    use crate::raw::{Serializable, functions};

    fn request(offset: &str, limit: i32) -> Vec<u8> {
        functions::payments::GetCraftStarGifts { gift_id: 1, offset: offset.into(), limit }.to_bytes()
    }

    #[tokio::test]
    async fn reads_the_gifts_across_pages() {
        let (client, mut sent) = Client::scripted(vec![Ok(page(2, 1, Some("next"))), Ok(page(2, 1, None))]);
        let gifts = client.get_gifts_for_crafting(1).await.unwrap();
        assert_eq!(ids(&gifts), ["10", "10"]);
        assert_eq!(sent.try_recv().unwrap(), request("", 100));
        assert_eq!(sent.try_recv().unwrap(), request("next", 100));
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn pages_with_offset_and_limit() {
        let (client, mut sent) = Client::scripted(vec![Ok(empty_page())]);
        let mut gifts = client.get_gifts_for_crafting(1).offset("x").limit(10);
        assert_eq!(gifts.next().await.unwrap(), None);
        assert_eq!(gifts.total().await.unwrap(), 0);
        assert_eq!(sent.try_recv().unwrap(), request("x", 10));
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_zero_limit_sends_nothing() {
        let (client, mut sent) = Client::recording();
        assert!(client.get_gifts_for_crafting(1).limit(0).await.unwrap().is_empty());
        assert!(sent.try_recv().is_err());
    }
}
