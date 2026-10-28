use super::owned_gift::{owned_gift_id, split_text};
use crate::dispatcher::Entities;
use crate::enums::UniqueGiftOrigin;
use crate::raw::{base, types};
use crate::types::{Gift, MessageEntity, UniqueGift};
use crate::utils::get_peer_id;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiftInfo {
    pub gift: Gift,
    pub owned_gift_id: Option<String>,
    pub convert_star_count: Option<i64>,
    pub prepaid_upgrade_star_count: Option<i64>,
    pub is_upgrade_separate: bool,
    pub can_be_upgraded: bool,
    pub text: Option<String>,
    pub entities: Option<Vec<MessageEntity>>,
    pub is_private: bool,
    pub unique_gift_number: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniqueGiftInfo {
    pub gift: UniqueGift,
    pub origin: UniqueGiftOrigin,
    pub last_resale_star_count: Option<i64>,
    pub last_resale_ton_amount: Option<i64>,
    pub owned_gift_id: Option<String>,
    pub transfer_star_count: Option<i64>,
    pub next_transfer_date: Option<i32>,
    pub text: Option<String>,
    pub entities: Option<Vec<MessageEntity>>,
    pub is_private: bool,
}

impl GiftInfo {
    pub(crate) fn parse(raw: &types::MessageActionStarGift, msg_id: i32, entities: &Entities) -> Option<GiftInfo> {
        let base::StarGift::StarGift(gift) = &raw.gift else { return None };
        let owner_id = raw.peer.as_ref().map_or(0, get_peer_id);
        let (text, text_entities) = split_text(raw.message.as_ref(), entities);
        Some(GiftInfo {
            gift: Gift::parse(gift, entities)?,
            owned_gift_id: owned_gift_id(owner_id, Some(msg_id), raw.saved_id),
            convert_star_count: raw.convert_stars,
            prepaid_upgrade_star_count: raw.upgrade_stars,
            is_upgrade_separate: raw.upgrade_separate,
            can_be_upgraded: raw.can_upgrade,
            text,
            entities: text_entities,
            is_private: raw.name_hidden,
            unique_gift_number: raw.gift_num,
        })
    }
}

impl UniqueGiftInfo {
    pub(crate) fn parse(
        raw: &types::MessageActionStarGiftUnique,
        msg_id: i32,
        entities: &Entities,
    ) -> Option<UniqueGiftInfo> {
        let base::StarGift::StarGiftUnique(gift) = &raw.gift else { return None };
        let origin = if raw.craft {
            UniqueGiftOrigin::Craft
        } else if raw.upgrade && raw.prepaid_upgrade {
            UniqueGiftOrigin::GiftedUpgrade
        } else if raw.upgrade {
            UniqueGiftOrigin::Upgrade
        } else if raw.from_offer {
            UniqueGiftOrigin::Offer
        } else if raw.resale_amount.is_some() {
            UniqueGiftOrigin::Resale
        } else {
            UniqueGiftOrigin::Transfer
        };
        let (last_resale_star_count, last_resale_ton_amount) = match &raw.resale_amount {
            Some(base::StarsAmount::StarsAmount(a)) => (Some(a.amount), None),
            Some(base::StarsAmount::StarsTonAmount(a)) => (None, Some(a.amount)),
            None => (None, None),
        };
        let owner_id = raw.peer.as_ref().map_or(0, get_peer_id);
        let (text, text_entities) = split_text(raw.message.as_ref(), entities);
        Some(UniqueGiftInfo {
            gift: UniqueGift::parse(gift, entities),
            origin,
            last_resale_star_count,
            last_resale_ton_amount,
            owned_gift_id: owned_gift_id(owner_id, Some(msg_id), raw.saved_id),
            transfer_star_count: raw.transfer_stars,
            next_transfer_date: raw.can_transfer_at,
            text,
            entities: text_entities,
            is_private: raw.name_hidden,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::UniqueGiftInfo;
    use crate::dispatcher::Entities;
    use crate::enums::UniqueGiftOrigin;
    use crate::raw::{base, types};

    fn action() -> types::MessageActionStarGiftUnique {
        let gift = types::StarGiftUnique {
            require_premium: false,
            resale_ton_only: false,
            theme_available: false,
            burned: false,
            crafted: false,
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
        };
        types::MessageActionStarGiftUnique {
            upgrade: false,
            transferred: false,
            saved: false,
            refunded: false,
            prepaid_upgrade: false,
            assigned: false,
            from_offer: false,
            craft: false,
            name_hidden: false,
            gift: base::StarGift::StarGiftUnique(Box::new(gift)),
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
        }
    }

    #[test]
    fn maps_unique_gift_origin() {
        let origin = |a: types::MessageActionStarGiftUnique| {
            let info = UniqueGiftInfo::parse(&a, 5, &Entities::default()).unwrap();
            (info.origin, info.last_resale_star_count, info.owned_gift_id)
        };
        assert_eq!(origin(action()), (UniqueGiftOrigin::Transfer, None, Some("5".into())));
        let upgrade = types::MessageActionStarGiftUnique { upgrade: true, ..action() };
        assert_eq!(origin(upgrade.clone()).0, UniqueGiftOrigin::Upgrade);
        let gifted = types::MessageActionStarGiftUnique { prepaid_upgrade: true, ..upgrade };
        assert_eq!(origin(gifted).0, UniqueGiftOrigin::GiftedUpgrade);
        let price = base::StarsAmount::StarsAmount(Box::new(types::StarsAmount { amount: 50, nanos: 0 }));
        let resale = types::MessageActionStarGiftUnique { resale_amount: Some(price), ..action() };
        assert_eq!(origin(resale), (UniqueGiftOrigin::Resale, Some(50), Some("5".into())));
        let channel = types::MessageActionStarGiftUnique {
            peer: Some(crate::utils::get_peer(-1000000000005)),
            saved_id: Some(9),
            ..action()
        };
        assert_eq!(origin(channel).2, Some("-1000000000005_9".into()));
    }
}
