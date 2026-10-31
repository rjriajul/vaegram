use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{CraftGiftResult, Message, ServiceAction};

impl Client {
    /// Crafts an upgraded gift from these owned gifts, which are used up whether or not it works.
    pub async fn craft_gift(&self, owned_gift_ids: &[&str]) -> Result<CraftGiftResult> {
        if owned_gift_ids.is_empty() {
            return Err(Error::InvalidArgument("owned_gift_ids is empty"));
        }
        let stargift = self.input_saved_gifts(owned_gift_ids).await?;
        let updates = self.invoke(&functions::payments::CraftStarGift { stargift }).await?;
        if craft_failed(&updates) {
            return Ok(CraftGiftResult::Failed);
        }
        Message::all_from_updates(self, updates)
            .find_map(|message| match message.service {
                Some(ServiceAction::UniqueGift(info)) => Some(CraftGiftResult::Crafted(info)),
                _ => None,
            })
            .ok_or(Error::Unexpected("payments.craftStarGift sent neither a gift nor a failure"))
    }
}

fn craft_failed(updates: &base::Updates) -> bool {
    let updates = match updates {
        base::Updates::Updates(u) => &u.updates[..],
        base::Updates::UpdatesCombined(u) => &u.updates[..],
        base::Updates::UpdateShort(u) => std::slice::from_ref(&u.update),
        _ => &[],
    };
    updates.iter().any(|u| matches!(u, base::Update::UpdateStarGiftCraftFail))
}

#[cfg(test)]
pub(super) mod tests {
    use crate::Client;
    use crate::enums::UniqueGiftOrigin;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::CraftGiftResult;
    use crate::utils::get_peer;

    pub(in crate::methods::payments) fn unique() -> base::StarGift {
        base::StarGift::StarGiftUnique(Box::new(types::StarGiftUnique {
            require_premium: false,
            resale_ton_only: false,
            theme_available: false,
            burned: false,
            crafted: true,
            id: 1,
            gift_id: 2,
            title: "Plush Pepe".into(),
            slug: "PlushPepe-3".into(),
            num: 3,
            owner_id: None,
            owner_name: None,
            owner_address: None,
            attributes: Vec::new(),
            availability_issued: 10,
            availability_total: 100,
            gift_address: None,
            resell_amount: None,
            released_by: None,
            value_amount: None,
            value_currency: None,
            value_usd_amount: None,
            theme_peer: None,
            peer_color: None,
            host_id: None,
            offer_min_stars: None,
            craft_chance_permille: None,
        }))
    }

    fn crafted() -> base::Update {
        let action = types::MessageActionStarGiftUnique {
            upgrade: false,
            transferred: false,
            saved: true,
            refunded: false,
            prepaid_upgrade: false,
            assigned: false,
            from_offer: false,
            craft: true,
            name_hidden: false,
            gift: unique(),
            can_export_at: None,
            transfer_stars: None,
            from_id: None,
            peer: None,
            saved_id: None,
            resale_amount: None,
            can_transfer_at: None,
            can_resell_at: None,
            drop_original_details_stars: None,
            can_craft_at: None,
            message: None,
        };
        let message = types::MessageService {
            out: false,
            mentioned: false,
            media_unread: false,
            reactions_are_possible: false,
            silent: false,
            post: false,
            legacy: false,
            id: 12,
            from_id: None,
            peer_id: get_peer(5),
            saved_peer_id: None,
            reply_to: None,
            date: 1,
            action: base::MessageAction::MessageActionStarGiftUnique(Box::new(action)),
            reactions: None,
            ttl_period: None,
        };
        let message = base::Message::MessageService(Box::new(message));
        base::Update::UpdateNewMessage(Box::new(types::UpdateNewMessage { message, pts: 1, pts_count: 1 }))
    }

    fn updates(update: base::Update) -> Vec<u8> {
        let updates = types::Updates { updates: vec![update], users: Vec::new(), chats: Vec::new(), date: 1, seq: 0 };
        base::Updates::Updates(Box::new(updates)).to_bytes()
    }

    #[tokio::test]
    async fn returns_the_crafted_gift() {
        let (client, mut sent) = Client::scripted(vec![Ok(updates(crafted()))]);
        let result = client.craft_gift(&["9", "10"]).await.unwrap();
        let CraftGiftResult::Crafted(info) = result else { panic!("expected a crafted gift, got {result:?}") };
        assert_eq!((info.origin, info.gift.name.as_str()), (UniqueGiftOrigin::Craft, "PlushPepe-3"));
        let stargift = [9, 10]
            .map(|msg_id| base::InputSavedStarGift::InputSavedStarGiftUser(types::InputSavedStarGiftUser { msg_id }))
            .to_vec();
        assert_eq!(sent.try_recv().unwrap(), functions::payments::CraftStarGift { stargift }.to_bytes());
    }

    #[tokio::test]
    async fn a_failed_roll_is_failed() {
        let (client, _sent) = Client::scripted(vec![Ok(updates(base::Update::UpdateStarGiftCraftFail))]);
        assert_eq!(client.craft_gift(&["9"]).await.unwrap(), CraftGiftResult::Failed);
    }

    #[tokio::test]
    async fn a_reply_without_either_is_unexpected() {
        let (client, _sent) = Client::scripted(vec![Ok(base::Updates::UpdatesTooLong.to_bytes())]);
        assert!(matches!(client.craft_gift(&["9"]).await, Err(Error::Unexpected(_))));
    }

    #[tokio::test]
    async fn no_gifts_is_refused_without_a_request() {
        let (client, mut sent) = Client::scripted(Vec::new());
        assert!(matches!(client.craft_gift(&[]).await, Err(Error::InvalidArgument(_))));
        assert!(sent.try_recv().is_err());
    }
}
