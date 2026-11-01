use crate::dispatcher::Entities;
use crate::enums::GiftPurchaseOfferState;
use crate::raw::{base, types};
use crate::types::{StarAmount, UniqueGift};

/// An offer to buy the receiver's upgraded gift.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradedGiftPurchaseOffer {
    pub gift: UniqueGift,
    pub state: GiftPurchaseOfferState,
    pub price: StarAmount,
    /// Unix time the offer runs out.
    pub expiration_date: i32,
}

/// An offer to buy an upgraded gift, turned down or left to run out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradedGiftPurchaseOfferRejected {
    pub gift: UniqueGift,
    pub price: StarAmount,
    pub was_expired: bool,
}

/// `None` for a regular gift: only upgraded gifts can be offered for.
fn unique(gift: &base::StarGift, entities: &Entities) -> Option<UniqueGift> {
    match gift {
        base::StarGift::StarGiftUnique(g) => Some(UniqueGift::parse(g, entities)),
        base::StarGift::StarGift(_) => None,
    }
}

impl UpgradedGiftPurchaseOffer {
    pub(crate) fn parse(
        raw: &types::MessageActionStarGiftPurchaseOffer,
        entities: &Entities,
    ) -> Option<UpgradedGiftPurchaseOffer> {
        let state = match (raw.accepted, raw.declined) {
            (true, _) => GiftPurchaseOfferState::Accepted,
            (_, true) => GiftPurchaseOfferState::Rejected,
            _ => GiftPurchaseOfferState::Pending,
        };
        Some(UpgradedGiftPurchaseOffer {
            gift: unique(&raw.gift, entities)?,
            state,
            price: StarAmount::parse(&raw.price),
            expiration_date: raw.expires_at,
        })
    }
}

impl UpgradedGiftPurchaseOfferRejected {
    pub(crate) fn parse(
        raw: &types::MessageActionStarGiftPurchaseOfferDeclined,
        entities: &Entities,
    ) -> Option<UpgradedGiftPurchaseOfferRejected> {
        Some(UpgradedGiftPurchaseOfferRejected {
            gift: unique(&raw.gift, entities)?,
            price: StarAmount::parse(&raw.price),
            was_expired: raw.expired,
        })
    }
}
