use super::get_chat_gifts::owned_gifts;
use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{OwnedGifts, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct GetGiftsForCrafting<'a> {
    client: &'a Client,
    request: functions::payments::GetCraftStarGifts,
}

impl Client {
    /// Lists the user's gifts that can go into crafting an upgraded `gift_id` gift.
    pub fn get_gifts_for_crafting(&self, gift_id: i64) -> GetGiftsForCrafting<'_> {
        let request = functions::payments::GetCraftStarGifts { gift_id, offset: String::new(), limit: 100 };
        GetGiftsForCrafting { client: self, request }
    }
}

impl GetGiftsForCrafting<'_> {
    pub fn offset(mut self, offset: impl Into<String>) -> Self {
        self.request.offset = offset.into();
        self
    }

    pub fn limit(mut self, limit: i32) -> Self {
        self.request.limit = limit;
        self
    }

    async fn send(self) -> Result<OwnedGifts> {
        let client = self.client;
        let (owner_id, _) = client.resolve(&PeerRef::Me).await?;
        let page = client.invoke(&self.request).await?;
        Ok(owned_gifts(client, page, owner_id))
    }
}

into_future!(GetGiftsForCrafting, OwnedGifts);

#[cfg(test)]
mod tests {
    use super::super::craft_gift::tests::unique;
    use super::super::get_chat_gifts::tests::empty_page;
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::OwnedGift;

    fn request(offset: &str, limit: i32) -> Vec<u8> {
        functions::payments::GetCraftStarGifts { gift_id: 1, offset: offset.into(), limit }.to_bytes()
    }

    fn page_of_one() -> Vec<u8> {
        let saved = types::SavedStarGift {
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
            msg_id: Some(9),
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
        };
        let page = types::payments::SavedStarGifts {
            count: 1,
            chat_notifications_enabled: None,
            gifts: vec![base::SavedStarGift::SavedStarGift(Box::new(saved))],
            next_offset: Some("next".into()),
            chats: Vec::new(),
            users: Vec::new(),
        };
        base::payments::SavedStarGifts::SavedStarGifts(Box::new(page)).to_bytes()
    }

    #[tokio::test]
    async fn lists_the_gifts_for_crafting() {
        let (client, mut sent) = Client::scripted(vec![Ok(page_of_one())]);
        let page = client.get_gifts_for_crafting(1).await.unwrap();
        assert_eq!(sent.try_recv().unwrap(), request("", 100));
        assert_eq!((page.total_count, page.next_offset.as_deref()), (1, Some("next")));
        let [OwnedGift::Unique(gift)] = &page.gifts[..] else { panic!("expected one upgraded gift") };
        assert_eq!(gift.owned_gift_id.as_deref(), Some("9"));
    }

    #[tokio::test]
    async fn pages_with_offset_and_limit() {
        let (client, mut sent) = Client::scripted(vec![Ok(empty_page())]);
        client.get_gifts_for_crafting(1).offset("x").limit(10).await.unwrap();
        assert_eq!(sent.try_recv().unwrap(), request("x", 10));
    }
}
