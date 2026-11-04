use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::methods::pages::OffsetPages;
use crate::raw::{base, functions};
use crate::types::{UniqueGift, UpgradedGiftAttributeId};

#[must_use = "a search does nothing until `next` is called"]
pub struct SearchGiftsForResale<'a> {
    client: &'a Client,
    request: functions::payments::GetResaleStarGifts,
    pages: OffsetPages<UniqueGift>,
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

    /// Only gifts with all of these models, symbols and backdrops.
    pub fn attributes(mut self, attributes: impl IntoIterator<Item = UpgradedGiftAttributeId>) -> Self {
        let attributes: Vec<_> = attributes.into_iter().map(UpgradedGiftAttributeId::write).collect();
        self.request.attributes = if attributes.is_empty() { None } else { Some(attributes) };
        self
    }

    /// Only gifts that can go into a craft.
    pub fn for_crafting(mut self) -> Self {
        self.request.for_craft = true;
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    pub async fn next(&mut self) -> Result<Option<UniqueGift>> {
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
                        base::StarGift::StarGiftUnique(g) => Some(UniqueGift::parse(g, &entities)),
                        base::StarGift::StarGift(_) => None,
                    })
                    .collect()
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::UpgradedGiftAttributeId;

    #[tokio::test]
    async fn search_sends_attribute_filters() {
        let (client, mut sent) = Client::scripted(Vec::new());
        let _ = client
            .search_gifts_for_resale(7)
            .attributes([
                UpgradedGiftAttributeId::Model(1),
                UpgradedGiftAttributeId::Symbol(2),
                UpgradedGiftAttributeId::Backdrop(3),
            ])
            .for_crafting()
            .next()
            .await;
        let attributes = vec![
            base::StarGiftAttributeId::StarGiftAttributeIdModel(types::StarGiftAttributeIdModel { document_id: 1 }),
            base::StarGiftAttributeId::StarGiftAttributeIdPattern(types::StarGiftAttributeIdPattern { document_id: 2 }),
            base::StarGiftAttributeId::StarGiftAttributeIdBackdrop(types::StarGiftAttributeIdBackdrop {
                backdrop_id: 3,
            }),
        ];
        let request = functions::payments::GetResaleStarGifts {
            sort_by_price: false,
            sort_by_num: false,
            for_craft: true,
            stars_only: false,
            attributes_hash: None,
            gift_id: 7,
            attributes: Some(attributes),
            offset: String::new(),
            limit: 100,
        };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
    }
}
