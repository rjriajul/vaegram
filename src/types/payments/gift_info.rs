use super::owned_gift::{owned_gift_id, split_text};
use crate::dispatcher::Entities;
use crate::enums::UniqueGiftOrigin;
use crate::raw::{base, types};
use crate::types::{Chat, Gift, MessageEntity, UniqueGift};
use crate::utils::get_peer_id;

/// A regular gift sent or received in a chat, taken from a service message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiftInfo {
    /// The gift.
    pub gift: Gift,
    /// Id that addresses the gift in calls such as [`Client::show_gift`](crate::Client::show_gift).
    /// `None` if the gift can't be addressed.
    pub owned_gift_id: Option<String>,
    /// Stars the receiver gets for converting the gift. `None` if Telegram doesn't send it.
    pub convert_star_count: Option<i64>,
    /// Stars the upgrade costs. `None` if Telegram doesn't send it.
    pub prepaid_upgrade_star_count: Option<i64>,
    /// Whether the upgrade is paid separately from the gift.
    pub is_upgrade_separate: bool,
    /// Whether the gift can be upgraded to a unique gift.
    pub can_be_upgraded: bool,
    /// Text the sender attached. `None` if there is none.
    pub text: Option<String>,
    /// Formatting entities of `text`.
    pub entities: Option<Vec<MessageEntity>>,
    /// Whether the sender's name is hidden.
    pub is_private: bool,
    /// Serial number of the unique gift. `None` if it isn't set.
    pub unique_gift_number: Option<i32>,
    /// Chat that sent the gift.
    pub sender: Option<Chat>,
    /// Chat that received the gift.
    pub receiver: Option<Chat>,
    /// Whether the gift is shown on the receiver's profile.
    pub is_saved: bool,
    /// Whether the gift was converted to Stars.
    pub was_converted: bool,
    /// Whether the gift was upgraded to a unique gift.
    pub was_upgraded: bool,
    /// Whether the gift was refunded.
    pub was_refunded: bool,
    /// Whether the upgrade was paid in advance.
    pub is_prepaid_upgrade: bool,
    /// Whether the gift was won in an auction.
    pub is_from_auction: bool,
    /// Hash that identifies the prepaid upgrade. `None` if there is none.
    pub prepaid_upgrade_hash: Option<String>,
    /// The service message that announced the upgrade.
    pub upgraded_message_id: Option<i32>,
    /// The message the gift was originally sent in.
    pub gift_message_id: Option<i32>,
}

/// A unique gift sent, received or changed, taken from a service message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniqueGiftInfo {
    /// The unique gift.
    pub gift: UniqueGift,
    /// How the gift came to this owner.
    pub origin: UniqueGiftOrigin,
    /// Stars the gift last sold for. `None` if it wasn't a Stars resale.
    pub last_resale_star_count: Option<i64>,
    /// TON amount the gift last sold for. `None` if it wasn't a TON resale.
    pub last_resale_ton_amount: Option<i64>,
    /// Id that addresses the gift in calls such as [`Client::show_gift`](crate::Client::show_gift).
    /// `None` if the gift can't be addressed.
    pub owned_gift_id: Option<String>,
    /// Stars a transfer costs. `None` if Telegram doesn't send it.
    pub transfer_star_count: Option<i64>,
    /// Unix time from which the gift can be transferred.
    pub next_transfer_date: Option<i32>,
    /// Text the sender attached. `None` if there is none.
    pub text: Option<String>,
    /// Formatting entities of `text`.
    pub entities: Option<Vec<MessageEntity>>,
    /// Whether the sender's name is hidden.
    pub is_private: bool,
    /// Chat that sent the gift.
    pub sender: Option<Chat>,
    /// Chat that received the gift.
    pub receiver: Option<Chat>,
    /// Whether the gift is shown on the receiver's profile.
    pub is_saved: bool,
    /// Whether the gift was refunded.
    pub was_refunded: bool,
    /// Whether the upgrade was paid in advance.
    pub is_prepaid_upgrade: bool,
    /// Unix time from which the gift can be exported.
    pub export_date: Option<i32>,
    /// Unix time from which the gift can be resold.
    pub next_resale_date: Option<i32>,
    /// Unix time from which the gift can be used for crafting.
    pub craft_date: Option<i32>,
    /// Stars it costs to remove the original details from the gift.
    pub drop_original_details_star_count: Option<i64>,
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
            sender: raw.from_id.as_ref().map(|p| Chat::parse(p, entities, true)),
            receiver: raw.to_id.as_ref().or(raw.peer.as_ref()).map(|p| Chat::parse(p, entities, true)),
            is_saved: raw.saved,
            was_converted: raw.converted,
            was_upgraded: raw.upgraded,
            was_refunded: raw.refunded,
            is_prepaid_upgrade: raw.prepaid_upgrade,
            is_from_auction: raw.auction_acquired,
            prepaid_upgrade_hash: raw.prepaid_upgrade_hash.clone(),
            upgraded_message_id: raw.upgrade_msg_id,
            gift_message_id: raw.gift_msg_id,
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
        // Flag precedence: assigned, craft, gifted upgrade, upgrade, offer, resale, then plain transfer.
        let origin = if raw.assigned {
            UniqueGiftOrigin::Blockchain
        } else if raw.craft {
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
            sender: raw.from_id.as_ref().map(|p| Chat::parse(p, entities, true)),
            receiver: raw.peer.as_ref().map(|p| Chat::parse(p, entities, true)),
            is_saved: raw.saved,
            was_refunded: raw.refunded,
            is_prepaid_upgrade: raw.prepaid_upgrade,
            export_date: raw.can_export_at,
            next_resale_date: raw.can_resell_at,
            craft_date: raw.can_craft_at,
            drop_original_details_star_count: raw.drop_original_details_stars,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{GiftInfo, UniqueGiftInfo};
    use crate::dispatcher::Entities;
    use crate::enums::UniqueGiftOrigin;
    use crate::raw::{base, types};
    use crate::types::messages_and_media::media::tests::document_media;

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

    #[test]
    fn assigned_gift_comes_from_the_blockchain() {
        let assigned = types::MessageActionStarGiftUnique { assigned: true, ..action() };
        let info = UniqueGiftInfo::parse(&assigned, 5, &Entities::default()).unwrap();
        assert_eq!(info.origin, UniqueGiftOrigin::Blockchain);
        let flags = types::MessageActionStarGiftUnique {
            saved: true,
            refunded: true,
            prepaid_upgrade: true,
            can_export_at: Some(1),
            can_resell_at: Some(2),
            can_craft_at: Some(3),
            drop_original_details_stars: Some(4),
            from_id: Some(crate::utils::get_peer(8)),
            peer: Some(crate::utils::get_peer(9)),
            ..action()
        };
        let info = UniqueGiftInfo::parse(&flags, 5, &Entities::default()).unwrap();
        assert!(info.is_saved && info.was_refunded && info.is_prepaid_upgrade);
        assert_eq!((info.export_date, info.next_resale_date, info.craft_date), (Some(1), Some(2), Some(3)));
        assert_eq!(info.drop_original_details_star_count, Some(4));
        assert_eq!((info.sender.map(|c| c.id), info.receiver.map(|c| c.id)), (Some(8), Some(9)));
    }

    fn regular_action() -> types::MessageActionStarGift {
        let attribute = base::DocumentAttribute::DocumentAttributeSticker(Box::new(types::DocumentAttributeSticker {
            mask: false,
            alt: String::new(),
            stickerset: base::InputStickerSet::InputStickerSetEmpty,
            mask_coords: None,
        }));
        let base::MessageMedia::MessageMediaDocument(media) = document_media("image/webp", vec![attribute]) else {
            panic!("expected a document")
        };
        let gift = types::StarGift {
            limited: false,
            sold_out: false,
            birthday: false,
            require_premium: false,
            limited_per_user: false,
            peer_color_available: false,
            auction: false,
            id: 1,
            sticker: media.document.expect("a sticker"),
            stars: 50,
            availability_remains: None,
            availability_total: None,
            availability_resale: None,
            convert_stars: 40,
            first_sale_date: None,
            last_sale_date: None,
            upgrade_stars: None,
            resell_min_stars: None,
            title: None,
            released_by: None,
            per_user_total: None,
            per_user_remains: None,
            locked_until_date: None,
            auction_slug: None,
            gifts_per_round: None,
            auction_start_date: None,
            upgrade_variants: None,
            background: None,
        };
        types::MessageActionStarGift {
            name_hidden: false,
            saved: true,
            converted: true,
            upgraded: true,
            refunded: true,
            can_upgrade: false,
            prepaid_upgrade: true,
            upgrade_separate: false,
            auction_acquired: true,
            gift: base::StarGift::StarGift(Box::new(gift)),
            message: None,
            convert_stars: None,
            upgrade_msg_id: Some(12),
            upgrade_stars: None,
            from_id: Some(crate::utils::get_peer(8)),
            peer: None,
            saved_id: None,
            prepaid_upgrade_hash: Some("h".into()),
            gift_msg_id: Some(13),
            to_id: Some(crate::utils::get_peer(9)),
            gift_num: None,
        }
    }

    #[test]
    fn gift_message_reads_sender_receiver_and_flags() {
        let raw = regular_action();
        let info = GiftInfo::parse(&raw, 5, &Entities::default()).unwrap();
        assert_eq!((info.sender.map(|c| c.id), info.receiver.map(|c| c.id)), (Some(8), Some(9)));
        assert!(info.is_saved && info.was_converted && info.was_upgraded && info.was_refunded);
        assert!(info.is_prepaid_upgrade && info.is_from_auction);
        assert_eq!(
            (info.prepaid_upgrade_hash.as_deref(), info.upgraded_message_id, info.gift_message_id),
            (Some("h"), Some(12), Some(13))
        );
    }

    #[test]
    fn gift_receiver_falls_back_to_the_peer() {
        let mut raw = regular_action();
        raw.to_id = None;
        raw.peer = Some(crate::utils::get_peer(7));
        let info = GiftInfo::parse(&raw, 5, &Entities::default()).unwrap();
        assert_eq!(info.receiver.map(|c| c.id), Some(7));
    }
}
