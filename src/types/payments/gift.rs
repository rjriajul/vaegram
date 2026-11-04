use super::{
    GiftAuction, GiftBackground, GiftResaleParameters, UpgradedGiftAttributeRarity, UpgradedGiftOriginalDetails,
};
use crate::dispatcher::Entities;
use crate::raw::{base, types};
use crate::types::{Chat, Media, Sticker};
use crate::utils::get_peer_id;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gift {
    pub id: i64,
    pub sticker: Sticker,
    pub star_count: i64,
    pub upgrade_star_count: Option<i64>,
    pub total_count: Option<i32>,
    pub remaining_count: Option<i32>,
    pub personal_total_count: Option<i32>,
    pub personal_remaining_count: Option<i32>,
    pub is_premium: bool,
    pub has_colors: bool,
    /// Sold only through an auction: see [`Client::get_gift_auction_state`](crate::Client::get_gift_auction_state).
    pub is_auction: bool,
    pub publisher_chat: Option<Chat>,
    pub title: Option<String>,
    /// Stars the gift's receiver gets for converting it.
    pub convert_star_count: i64,
    /// How many of this gift are listed for resale.
    pub available_resale_count: Option<i64>,
    pub minimum_resell_star_count: Option<i64>,
    /// How many models, symbols and backdrops the gift can be upgraded into.
    pub unique_gift_variant_count: Option<i32>,
    pub first_sale_date: Option<i32>,
    pub last_sale_date: Option<i32>,
    pub locked_until_date: Option<i32>,
    pub is_limited: bool,
    pub is_sold_out: bool,
    pub is_for_birthday: bool,
    pub is_limited_per_user: bool,
    pub auction_info: Option<GiftAuction>,
    pub background: Option<GiftBackground>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UniqueGift {
    /// This unique gift's own id; `gift_id` is the regular gift it was upgraded from.
    pub id: i64,
    pub gift_id: i64,
    pub base_name: String,
    pub name: String,
    pub number: i32,
    pub model: Option<UniqueGiftAttribute>,
    pub symbol: Option<UniqueGiftAttribute>,
    pub backdrop: Option<UniqueGiftBackdrop>,
    pub is_premium: bool,
    pub is_burned: bool,
    pub publisher_chat: Option<Chat>,
    pub owner: Option<Chat>,
    pub owner_name: Option<String>,
    pub owner_address: Option<String>,
    pub gift_address: Option<String>,
    pub host: Option<Chat>,
    pub is_crafted: bool,
    pub is_theme_available: bool,
    /// How many of this gift have been upgraded so far.
    pub total_upgraded_count: i32,
    /// How many of this gift can be upgraded at most.
    pub max_upgraded_count: i32,
    pub minimum_offer_star_count: Option<i32>,
    pub used_theme_chat_id: Option<i64>,
    pub craft_probability_per_mille: Option<i32>,
    pub value_amount: Option<i64>,
    pub value_currency: Option<String>,
    pub value_usd_amount: Option<i64>,
    pub original_details: Option<UpgradedGiftOriginalDetails>,
    pub resale_parameters: Option<GiftResaleParameters>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniqueGiftAttribute {
    pub name: String,
    pub sticker: Sticker,
    pub rarity: UpgradedGiftAttributeRarity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniqueGiftBackdrop {
    pub name: String,
    pub center_color: i32,
    pub edge_color: i32,
    pub symbol_color: i32,
    pub text_color: i32,
    pub rarity: UpgradedGiftAttributeRarity,
}

impl Gift {
    pub(crate) fn parse(raw: &types::StarGift, entities: &Entities) -> Option<Gift> {
        Some(Gift {
            id: raw.id,
            sticker: sticker(&raw.sticker)?,
            star_count: raw.stars,
            upgrade_star_count: raw.upgrade_stars,
            total_count: raw.availability_total,
            remaining_count: raw.availability_remains,
            personal_total_count: raw.per_user_total,
            personal_remaining_count: raw.per_user_remains,
            is_premium: raw.require_premium,
            has_colors: raw.peer_color_available,
            is_auction: raw.auction,
            publisher_chat: raw.released_by.as_ref().map(|p| Chat::parse(p, entities, true)),
            title: raw.title.clone(),
            convert_star_count: raw.convert_stars,
            available_resale_count: raw.availability_resale,
            minimum_resell_star_count: raw.resell_min_stars,
            unique_gift_variant_count: raw.upgrade_variants,
            first_sale_date: raw.first_sale_date,
            last_sale_date: raw.last_sale_date,
            locked_until_date: raw.locked_until_date,
            is_limited: raw.limited,
            is_sold_out: raw.sold_out,
            is_for_birthday: raw.birthday,
            is_limited_per_user: raw.limited_per_user,
            auction_info: match (&raw.auction_slug, raw.gifts_per_round, raw.auction_start_date) {
                (Some(slug), Some(gifts_per_round), Some(start_date)) => {
                    Some(GiftAuction { slug: slug.clone(), gifts_per_round, start_date })
                }
                _ => None,
            },
            background: raw.background.as_ref().map(GiftBackground::parse),
        })
    }
}

impl UniqueGift {
    pub(crate) fn parse(raw: &types::StarGiftUnique, entities: &Entities) -> UniqueGift {
        let (mut model, mut symbol, mut backdrop, mut original_details) = (None, None, None, None);
        for raw_attribute in &raw.attributes {
            match raw_attribute {
                base::StarGiftAttribute::StarGiftAttributeModel(a) => {
                    model = attribute(&a.name, &a.document, &a.rarity)
                }
                base::StarGiftAttribute::StarGiftAttributePattern(a) => {
                    symbol = attribute(&a.name, &a.document, &a.rarity)
                }
                base::StarGiftAttribute::StarGiftAttributeBackdrop(a) => backdrop = Some(self::backdrop(a)),
                base::StarGiftAttribute::StarGiftAttributeOriginalDetails(a) => {
                    original_details = Some(UpgradedGiftOriginalDetails::parse(a, entities))
                }
            }
        }
        UniqueGift {
            id: raw.id,
            gift_id: raw.gift_id,
            base_name: raw.title.clone(),
            name: raw.slug.clone(),
            number: raw.num,
            model,
            symbol,
            backdrop,
            is_premium: raw.require_premium,
            is_burned: raw.burned,
            publisher_chat: raw.released_by.as_ref().map(|p| Chat::parse(p, entities, true)),
            owner: raw.owner_id.as_ref().map(|p| Chat::parse(p, entities, true)),
            owner_name: raw.owner_name.clone(),
            owner_address: raw.owner_address.clone(),
            gift_address: raw.gift_address.clone(),
            host: raw.host_id.as_ref().map(|p| Chat::parse(p, entities, true)),
            is_crafted: raw.crafted,
            is_theme_available: raw.theme_available,
            total_upgraded_count: raw.availability_issued,
            max_upgraded_count: raw.availability_total,
            minimum_offer_star_count: raw.offer_min_stars,
            used_theme_chat_id: raw.theme_peer.as_ref().map(get_peer_id),
            craft_probability_per_mille: raw.craft_chance_permille,
            value_amount: raw.value_amount,
            value_currency: raw.value_currency.clone(),
            value_usd_amount: raw.value_usd_amount,
            original_details,
            resale_parameters: GiftResaleParameters::parse(raw.resell_amount.as_deref(), raw.resale_ton_only),
        }
    }

    /// Whether a purchase offer can be sent: it requires an owner, and a gift that accepts offers.
    pub fn can_send_purchase_offer(&self) -> bool {
        self.owner.is_some() && self.minimum_offer_star_count.is_some()
    }

    /// The public link to the gift.
    pub fn link(&self) -> String {
        format!("https://t.me/nft/{}", self.name)
    }
}

pub(super) fn attribute(
    name: &str,
    document: &base::Document,
    rarity: &base::StarGiftAttributeRarity,
) -> Option<UniqueGiftAttribute> {
    Some(UniqueGiftAttribute {
        name: name.to_owned(),
        sticker: sticker(document)?,
        rarity: UpgradedGiftAttributeRarity::parse(rarity),
    })
}

pub(super) fn backdrop(raw: &types::StarGiftAttributeBackdrop) -> UniqueGiftBackdrop {
    UniqueGiftBackdrop {
        name: raw.name.clone(),
        center_color: raw.center_color,
        edge_color: raw.edge_color,
        symbol_color: raw.pattern_color,
        text_color: raw.text_color,
        rarity: UpgradedGiftAttributeRarity::parse(&raw.rarity),
    }
}

pub(super) fn sticker(document: &base::Document) -> Option<Sticker> {
    match Media::from_document(document)? {
        Media::Sticker(sticker) => Some(sticker),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{Gift, UniqueGift};
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};
    use crate::types::messages_and_media::media::tests::document_media;
    use crate::types::payments::UpgradedGiftAttributeRarity;

    fn sticker() -> base::Document {
        let attribute = base::DocumentAttribute::DocumentAttributeSticker(Box::new(types::DocumentAttributeSticker {
            mask: false,
            alt: String::new(),
            stickerset: base::InputStickerSet::InputStickerSetEmpty,
            mask_coords: None,
        }));
        let base::MessageMedia::MessageMediaDocument(media) = document_media("image/webp", vec![attribute]) else {
            panic!("expected a document")
        };
        media.document.expect("a sticker")
    }

    fn regular() -> types::StarGift {
        types::StarGift {
            limited: false,
            sold_out: false,
            birthday: false,
            require_premium: false,
            limited_per_user: false,
            peer_color_available: false,
            auction: false,
            id: 1,
            sticker: sticker(),
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
        }
    }

    fn unique() -> types::StarGiftUnique {
        types::StarGiftUnique {
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
        }
    }

    #[test]
    fn unique_gifts_keep_their_own_id() {
        let gift = UniqueGift::parse(&unique(), &Entities::default());
        assert_eq!((gift.id, gift.gift_id, gift.number), (1, 2, 3));
    }

    #[test]
    fn reads_regular_gift_limits_auction_and_background() {
        let raw = types::StarGift {
            limited: true,
            sold_out: true,
            birthday: true,
            limited_per_user: true,
            title: Some("Heart".into()),
            availability_resale: Some(7),
            resell_min_stars: Some(30),
            first_sale_date: Some(100),
            last_sale_date: Some(200),
            locked_until_date: Some(300),
            upgrade_variants: Some(12),
            auction_slug: Some("heart-auction".into()),
            gifts_per_round: Some(5),
            auction_start_date: Some(400),
            background: Some(base::StarGiftBackground::StarGiftBackground(Box::new(types::StarGiftBackground {
                center_color: 1,
                edge_color: 2,
                text_color: 3,
            }))),
            ..regular()
        };
        let gift = Gift::parse(&raw, &Entities::default()).unwrap();
        assert_eq!((gift.title.as_deref(), gift.convert_star_count), (Some("Heart"), 40));
        assert_eq!((gift.available_resale_count, gift.minimum_resell_star_count), (Some(7), Some(30)));
        assert_eq!(
            (gift.first_sale_date, gift.last_sale_date, gift.locked_until_date),
            (Some(100), Some(200), Some(300))
        );
        assert_eq!(gift.unique_gift_variant_count, Some(12));
        assert!(gift.is_limited && gift.is_sold_out && gift.is_for_birthday && gift.is_limited_per_user);
        let auction = gift.auction_info.unwrap();
        assert_eq!((auction.slug.as_str(), auction.gifts_per_round, auction.start_date), ("heart-auction", 5, 400));
        let background = gift.background.unwrap();
        assert_eq!((background.center_color, background.edge_color, background.text_color), (1, 2, 3));
        assert_eq!(Gift::parse(&regular(), &Entities::default()).unwrap().auction_info, None);
    }

    #[test]
    fn reads_owner_value_and_original_details() {
        let details = types::StarGiftAttributeOriginalDetails {
            sender_id: Some(crate::utils::get_peer(8)),
            recipient_id: crate::utils::get_peer(9),
            date: 77,
            message: Some(base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities {
                text: "hi".into(),
                entities: Vec::new(),
            }))),
        };
        let raw = types::StarGiftUnique {
            owner_id: Some(crate::utils::get_peer(9)),
            owner_name: Some("Ann".into()),
            owner_address: Some("EQ".into()),
            gift_address: Some("EQG".into()),
            host_id: Some(crate::utils::get_peer(10)),
            crafted: true,
            theme_available: true,
            theme_peer: Some(crate::utils::get_peer(11)),
            offer_min_stars: Some(25),
            craft_chance_permille: Some(40),
            value_amount: Some(500),
            value_currency: Some("USD".into()),
            value_usd_amount: Some(499),
            attributes: vec![base::StarGiftAttribute::StarGiftAttributeOriginalDetails(Box::new(details))],
            ..unique()
        };
        let gift = UniqueGift::parse(&raw, &Entities::default());
        assert_eq!(gift.owner.as_ref().map(|c| c.id), Some(9));
        assert_eq!(gift.host.as_ref().map(|c| c.id), Some(10));
        assert_eq!((gift.owner_name.as_deref(), gift.owner_address.as_deref()), (Some("Ann"), Some("EQ")));
        assert_eq!(gift.gift_address.as_deref(), Some("EQG"));
        assert!(gift.is_crafted && gift.is_theme_available);
        assert_eq!((gift.total_upgraded_count, gift.max_upgraded_count), (10, 100));
        assert_eq!((gift.used_theme_chat_id, gift.minimum_offer_star_count), (Some(11), Some(25)));
        assert_eq!(gift.craft_probability_per_mille, Some(40));
        assert_eq!(
            (gift.value_amount, gift.value_currency.as_deref(), gift.value_usd_amount),
            (Some(500), Some("USD"), Some(499))
        );
        let original = gift.original_details.as_ref().unwrap();
        assert_eq!(
            (original.sender.as_ref().map(|c| c.id), original.receiver.id, original.date, original.text.as_deref()),
            (Some(8), 9, 77, Some("hi"))
        );
        assert!(gift.can_send_purchase_offer());
        assert!(!UniqueGift::parse(&unique(), &Entities::default()).can_send_purchase_offer());
        assert_eq!(gift.link(), "https://t.me/nft/PlushPepe-3");
    }

    #[test]
    fn attributes_keep_their_rarity_tier() {
        let model = types::StarGiftAttributeModel {
            crafted: false,
            name: "Pepe".into(),
            document: sticker(),
            rarity: base::StarGiftAttributeRarity::StarGiftAttributeRarityLegendary,
        };
        let raw = types::StarGiftUnique {
            attributes: vec![base::StarGiftAttribute::StarGiftAttributeModel(Box::new(model))],
            ..unique()
        };
        let gift = UniqueGift::parse(&raw, &Entities::default());
        assert_eq!(gift.model.unwrap().rarity, UpgradedGiftAttributeRarity::Legendary);
    }
}
