use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::methods::pages::OffsetPages;
use crate::raw::{base, functions};
use crate::types::ResaleGift;

pub struct SearchGiftsForResale<'a> {
    client: &'a Client,
    request: functions::payments::GetResaleStarGifts,
    pages: OffsetPages<ResaleGift>,
}

impl Client {
    pub fn search_gifts_for_resale(&self, gift_id: i64) -> SearchGiftsForResale<'_> {
        let request = functions::payments::GetResaleStarGifts {
            sort_by_price: false,
            sort_by_num: false,
            for_craft: false,
            stars_only: false,
            attributes_hash: None,
            gift_id,
            attributes: None,
            offset: String::new(),
            limit: 100,
        };
        SearchGiftsForResale { client: self, request, pages: OffsetPages::new(100) }
    }
}

impl SearchGiftsForResale<'_> {
    pub fn sort_by_price(mut self) -> Self {
        self.request.sort_by_price = true;
        self
    }

    pub fn sort_by_number(mut self) -> Self {
        self.request.sort_by_num = true;
        self
    }

    pub fn stars_only(mut self) -> Self {
        self.request.stars_only = true;
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    pub async fn next(&mut self) -> Result<Option<ResaleGift>> {
        loop {
            if let Some(gift) = self.pages.pop() {
                return Ok(Some(gift));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            self.request.offset = self.pages.next_offset().unwrap_or_default().into();
            self.request.limit = limit;
            let base::payments::ResaleStarGifts::ResaleStarGifts(page) = self.client.invoke(&self.request).await?;
            let page = *page;
            let entities = Entities::new(self.client, page.users, page.chats);
            self.pages.refill(page.gifts, page.next_offset, |gifts| {
                gifts
                    .iter()
                    .filter_map(|g| match g {
                        base::StarGift::StarGiftUnique(g) => Some(ResaleGift::parse(g, &entities)),
                        base::StarGift::StarGift(_) => None,
                    })
                    .collect()
            });
        }
    }
}
