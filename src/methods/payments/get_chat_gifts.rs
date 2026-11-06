use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::methods::pages::OffsetPages;
use crate::raw::{base, functions};
use crate::types::{OwnedGift, PeerRef};

/// Gifts on a profile, built by [`Client::get_chat_gifts`], [`Client::get_user_gifts`] and
/// [`Client::get_business_account_gifts`].
///
/// Awaiting it collects the gifts into a `Vec`, up to [`limit`](GetChatGifts::limit);
/// [`next`](GetChatGifts::next) reads them one at a time. Gifts are fetched 100 to a page.
#[must_use = "a pager does nothing until it is awaited or read with next()"]
pub struct GetChatGifts<'a> {
    client: &'a Client,
    chat_id: Option<PeerRef>,
    connection_id: Option<String>,
    request: functions::payments::GetSavedStarGifts,
    // The id gift ids are built from, once the chat is resolved.
    owner_id: Option<i64>,
    total: Option<i32>,
    pages: OffsetPages<OwnedGift>,
}

impl Client {
    /// Lists the gifts on a chat's profile.
    pub fn get_chat_gifts(&self, chat_id: impl Into<PeerRef>) -> GetChatGifts<'_> {
        GetChatGifts::new(self, Some(chat_id.into()), None)
    }
}

impl<'a> GetChatGifts<'a> {
    pub(crate) fn new(client: &'a Client, chat_id: Option<PeerRef>, connection_id: Option<String>) -> Self {
        let request = functions::payments::GetSavedStarGifts {
            exclude_unsaved: false,
            exclude_saved: false,
            exclude_unlimited: false,
            exclude_unique: false,
            sort_by_value: false,
            exclude_upgradable: false,
            exclude_unupgradable: false,
            peer_color_available: false,
            exclude_hosted: false,
            peer: base::InputPeer::InputPeerSelf,
            collection_id: None,
            offset: String::new(),
            limit: 0,
        };
        let pages = OffsetPages::new(100);
        GetChatGifts { client, chat_id, connection_id, request, owner_id: None, total: None, pages }
    }

    /// Leaves out gifts that are hidden from the profile.
    pub fn exclude_unsaved(mut self) -> Self {
        self.request.exclude_unsaved = true;
        self
    }

    /// Leaves out gifts that are shown on the profile.
    pub fn exclude_saved(mut self) -> Self {
        self.request.exclude_saved = true;
        self
    }

    /// Leaves out gifts that have no limited supply.
    pub fn exclude_unlimited(mut self) -> Self {
        self.request.exclude_unlimited = true;
        self
    }

    /// Leaves out limited gifts that can still be upgraded.
    pub fn exclude_limited_upgradable(mut self) -> Self {
        self.request.exclude_upgradable = true;
        self
    }

    /// Leaves out limited gifts that can't be upgraded.
    pub fn exclude_limited_non_upgradable(mut self) -> Self {
        self.request.exclude_unupgradable = true;
        self
    }

    /// Leaves out unique gifts, the ones that were already upgraded.
    pub fn exclude_unique(mut self) -> Self {
        self.request.exclude_unique = true;
        self
    }

    /// Orders gifts by their value.
    pub fn sort_by_price(mut self) -> Self {
        self.request.sort_by_value = true;
        self
    }

    /// Starts at this offset token, taken from [`next_offset`](Self::next_offset) of an earlier pager, instead of
    /// the first gift.
    pub fn offset(mut self, offset: impl Into<String>) -> Self {
        self.pages.set_offset(offset.into());
        self
    }

    /// Where the unread part of the list starts, for `offset` on a later pager; `None` once the list has ended.
    /// Gifts already fetched but not yet returned by `next` come before it.
    pub fn next_offset(&self) -> Option<&str> {
        self.pages.offset().map(String::as_str)
    }

    /// Stops after this many gifts in total. The default reads the whole list.
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    /// Lists only the gifts in this collection.
    pub fn collection(mut self, collection_id: i32) -> Self {
        self.request.collection_id = Some(collection_id);
        self
    }

    /// Returns how many gifts match the filters in all, ignoring `limit` and `offset`.
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
        let owner_id = match (self.owner_id, &self.chat_id) {
            (Some(id), _) => id,
            (None, Some(chat)) => {
                let (id, peer) = client.resolve(chat).await?;
                self.request.peer = peer;
                id
            }
            // No chat leaves InputPeerSelf, which through a business connection is the business account.
            (None, None) => 0,
        };
        self.owner_id = Some(owner_id);
        self.request.offset = self.pages.next_offset().unwrap_or_default().into();
        self.request.limit = limit;
        let (page, _) = client.invoke_business(self.connection_id.as_deref(), self.request.clone()).await?;
        self.total = Some(push_gifts(client, &mut self.pages, page, owner_id));
        Ok(())
    }
}

pager!(GetChatGifts, OwnedGift);

/// Buffers a page of saved gifts and returns how many the whole list holds.
pub(super) fn push_gifts(
    client: &Client,
    pages: &mut OffsetPages<OwnedGift>,
    page: base::payments::SavedStarGifts,
    owner_id: i64,
) -> i32 {
    let base::payments::SavedStarGifts::SavedStarGifts(page) = page;
    let page = *page;
    let entities = Entities::new(client, page.users, page.chats);
    pages.refill(page.gifts, page.next_offset, |gifts| {
        gifts
            .iter()
            .filter_map(|g| match g {
                base::SavedStarGift::SavedStarGift(g) => OwnedGift::parse(g, owner_id, &entities),
            })
            .collect()
    });
    page.count
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::craft_gift::tests::unique;
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::OwnedGift;

    pub(in crate::methods::payments) fn empty_page() -> Vec<u8> {
        page(0, 0, None)
    }

    /// A page of `gifts` upgraded gifts with message ids from 10 on, out of `count` in all.
    pub(in crate::methods::payments) fn page(count: i32, gifts: i32, next_offset: Option<&str>) -> Vec<u8> {
        let gifts = (0..gifts).map(|i| base::SavedStarGift::SavedStarGift(Box::new(saved(10 + i)))).collect();
        let page = types::payments::SavedStarGifts {
            count,
            chat_notifications_enabled: None,
            gifts,
            next_offset: next_offset.map(Into::into),
            chats: Vec::new(),
            users: Vec::new(),
        };
        base::payments::SavedStarGifts::SavedStarGifts(Box::new(page)).to_bytes()
    }

    fn saved(msg_id: i32) -> types::SavedStarGift {
        types::SavedStarGift {
            name_hidden: false,
            unsaved: false,
            refunded: false,
            can_upgrade: false,
            pinned_to_top: false,
            upgrade_separate: false,
            from_id: None,
            date: 1,
            gift: unique(),
            message: None,
            msg_id: Some(msg_id),
            saved_id: None,
            convert_stars: None,
            upgrade_stars: None,
            can_export_at: None,
            transfer_stars: None,
            can_transfer_at: None,
            can_resell_at: None,
            collection_id: None,
            prepaid_upgrade_hash: None,
            drop_original_details_stars: None,
            gift_num: None,
            can_craft_at: None,
        }
    }

    pub(in crate::methods::payments) fn ids(gifts: &[OwnedGift]) -> Vec<String> {
        let id = |g: &OwnedGift| match g {
            OwnedGift::Unique(g) => g.owned_gift_id.clone().unwrap_or_default(),
            OwnedGift::Regular(g) => g.owned_gift_id.clone().unwrap_or_default(),
        };
        gifts.iter().map(id).collect()
    }

    fn request(offset: &str, limit: i32) -> functions::payments::GetSavedStarGifts {
        let mut request = Client::offline().get_chat_gifts("me").request;
        (request.offset, request.limit) = (offset.into(), limit);
        request
    }

    #[tokio::test]
    async fn lists_one_collection() {
        let (client, mut sent) = Client::scripted(vec![Ok(empty_page())]);
        assert!(client.get_chat_gifts("me").collection(4).await.unwrap().is_empty());
        let mut request = request("", 100);
        request.collection_id = Some(4);
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn pages_with_the_offset_telegram_hands_out() {
        let replies = vec![Ok(page(3, 2, Some("p2"))), Ok(page(3, 1, None))];
        let (client, mut sent) = Client::scripted(replies);
        let gifts = client.get_chat_gifts("me").offset("p1").await.unwrap();
        assert_eq!(ids(&gifts), ["10", "11", "10"]);
        assert_eq!(sent.try_recv().unwrap(), request("p1", 100).to_bytes());
        assert_eq!(sent.try_recv().unwrap(), request("p2", 100).to_bytes());
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn the_limit_caps_the_pages_and_the_gifts() {
        let (client, mut sent) = Client::scripted(vec![Ok(page(9, 2, Some("p2"))), Ok(page(9, 1, Some("p3")))]);
        let mut gifts = client.get_chat_gifts("me").limit(3);
        assert_eq!(gifts.total().await.unwrap(), 9);
        let mut got = Vec::new();
        while let Some(gift) = gifts.next().await.unwrap() {
            got.push(gift);
        }
        assert_eq!(ids(&got), ["10", "11", "10"]);
        assert_eq!(sent.try_recv().unwrap(), request("", 3).to_bytes());
        assert_eq!(sent.try_recv().unwrap(), request("p2", 1).to_bytes());
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn next_offset_is_where_a_later_pager_resumes() {
        let (client, mut sent) = Client::scripted(vec![Ok(page(9, 2, Some("p2"))), Ok(empty_page())]);
        let mut gifts = client.get_chat_gifts("me").limit(2);
        assert_eq!(gifts.next_offset(), Some(""));
        while gifts.next().await.unwrap().is_some() {}
        assert_eq!(gifts.next_offset(), Some("p2"));
        let mut rest = client.get_chat_gifts("me").offset(gifts.next_offset().unwrap());
        assert!(rest.next().await.unwrap().is_none());
        assert_eq!(rest.next_offset(), None);
        assert_eq!(sent.try_recv().unwrap(), request("", 2).to_bytes());
        assert_eq!(sent.try_recv().unwrap(), request("p2", 100).to_bytes());
        assert!(sent.try_recv().is_err());
    }
}
