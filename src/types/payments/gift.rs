use super::{
    GiftAuction, GiftBackground, GiftResaleParameters, UpgradedGiftAttributeRarity, UpgradedGiftOriginalDetails,
};
use crate::dispatcher::Entities;
use crate::raw::{base, types};
use crate::types::{Chat, Media, Sticker};
use crate::utils::get_peer_id;

/// A regular gift that can be sent to a chat.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Gift {
    /// Id of the gift.
    pub id: i64,
    /// Sticker that shows the gift.
    pub sticker: Sticker,
    /// Price in Stars.
    pub star_count: i64,
    /// Stars it costs to upgrade the gift to a [`UniqueGift`].
    pub upgrade_star_count: Option<i64>,
    /// Total supply of a limited gift. `None` if the gift is not limited.
    pub total_count: Option<i32>,
    /// Copies of a limited gift still available. `None` if the gift is not limited.
    pub remaining_count: Option<i32>,
    /// How many copies one user can buy. `None` if the gift is not limited per user.
    pub personal_total_count: Option<i32>,
    /// How many copies the current user can still buy. `None` if the gift is not limited per user.
    pub personal_remaining_count: Option<i32>,
    /// Whether only Premium users can send the gift.
    pub is_premium: bool,
    /// Whether the gift can be used for a peer color.
    pub has_colors: bool,
    /// Sold only through an auction: see [`Client::get_gift_auction_state`](crate::Client::get_gift_auction_state).
    pub is_auction: bool,
    /// Chat that released the gift.
    pub publisher_chat: Option<Chat>,
    /// Title of the gift.
    pub title: Option<String>,
    /// Stars the gift's receiver gets for converting it.
    pub convert_star_count: i64,
    /// How many of this gift are listed for resale.
    pub available_resale_count: Option<i64>,
    /// Lowest resale price in Stars.
    pub minimum_resell_star_count: Option<i64>,
    /// How many models, symbols and backdrops the gift can be upgraded into.
    pub unique_gift_variant_count: Option<i32>,
    /// Unix time of the first sale.
    pub first_sale_date: Option<i32>,
    /// Unix time of the last sale.
    pub last_sale_date: Option<i32>,
    /// Unix time until which the gift is locked.
    pub locked_until_date: Option<i32>,
    /// Whether the supply is limited.
    pub is_limited: bool,
    /// Whether no copies are left.
    pub is_sold_out: bool,
    /// Whether the gift is a birthday gift.
    pub is_for_birthday: bool,
    /// Whether each user can buy only a limited number of copies.
    pub is_limited_per_user: bool,
    /// Auction details. `None` unless Telegram sends the auction's slug, round size and start date.
    pub auction_info: Option<GiftAuction>,
    /// Colors the gift is drawn on.
    pub background: Option<GiftBackground>,
}

/// A unique gift, upgraded from a regular [`Gift`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UniqueGift {
    /// Id of this unique gift.
    pub id: i64,
    /// Id of the regular gift this one was upgraded from.
    pub gift_id: i64,
    /// Title of the gift.
    pub base_name: String,
    /// Slug of the gift, as used in its public link.
    pub name: String,
    /// Serial number among the upgraded copies.
    pub number: i32,
    /// Model of the gift. `None` if Telegram sends none or its document isn't a sticker.
    pub model: Option<UniqueGiftAttribute>,
    /// Symbol of the gift. `None` if Telegram sends none or its document isn't a sticker.
    pub symbol: Option<UniqueGiftAttribute>,
    /// Backdrop of the gift.
    pub backdrop: Option<UniqueGiftBackdrop>,
    /// Whether only Premium users can send the gift.
    pub is_premium: bool,
    /// Whether the gift was burned.
    pub is_burned: bool,
    /// Chat that released the gift.
    pub publisher_chat: Option<Chat>,
    /// Chat that owns the gift.
    pub owner: Option<Chat>,
    /// Name of the owner.
    pub owner_name: Option<String>,
    /// TON address of the owner.
    pub owner_address: Option<String>,
    /// TON address of the gift.
    pub gift_address: Option<String>,
    /// Chat that hosts the gift.
    pub host: Option<Chat>,
    /// Whether the gift was made by crafting.
    pub is_crafted: bool,
    /// Whether the gift can be used as a chat theme.
    pub is_theme_available: bool,
    /// How many of this gift have been upgraded so far.
    pub total_upgraded_count: i32,
    /// How many of this gift can be upgraded at most.
    pub max_upgraded_count: i32,
    /// Lowest purchase offer in Stars. `None` if the gift takes no offers.
    pub minimum_offer_star_count: Option<i32>,
    /// Id of the chat using the gift as its theme. `None` if none does.
    pub used_theme_chat_id: Option<i64>,
    /// Chance of crafting this gift, in parts per thousand.
    pub craft_probability_per_mille: Option<i32>,
    /// Estimated value of the gift, in `value_currency`.
    pub value_amount: Option<i64>,
    /// Currency of `value_amount`.
    pub value_currency: Option<String>,
    /// Estimated value of the gift in US dollars, as Telegram sends it.
    pub value_usd_amount: Option<i64>,
    /// Details of the gift as first sent.
    pub original_details: Option<UpgradedGiftOriginalDetails>,
    /// Resale price. `None` if the gift isn't listed for sale.
    pub resale_parameters: Option<GiftResaleParameters>,
}

/// A model or symbol of a unique gift.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UniqueGiftAttribute {
    /// Name of the attribute.
    pub name: String,
    /// Sticker that shows the attribute.
    pub sticker: Sticker,
    /// How rare the attribute is.
    pub rarity: UpgradedGiftAttributeRarity,
}

/// The backdrop of a unique gift.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UniqueGiftBackdrop {
    /// Name of the backdrop.
    pub name: String,
    /// Color at the center, as an RGB integer.
    pub center_color: i32,
    /// Color at the edges, as an RGB integer.
    pub edge_color: i32,
    /// Color of the symbols drawn on the backdrop, as an RGB integer.
    pub symbol_color: i32,
    /// Color of text drawn over the backdrop, as an RGB integer.
    pub text_color: i32,
    /// How rare the backdrop is.
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
    /// Every field zero, empty or `None`: the base the parsers and tests fill in.
    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        UniqueGift {
            id: 0,
            gift_id: 0,
            base_name: String::new(),
            name: String::new(),
            number: 0,
            model: None,
            symbol: None,
            backdrop: None,
            is_premium: false,
            is_burned: false,
            publisher_chat: None,
            owner: None,
            owner_name: None,
            owner_address: None,
            gift_address: None,
            host: None,
            is_crafted: false,
            is_theme_available: false,
            total_upgraded_count: 0,
            max_upgraded_count: 0,
            minimum_offer_star_count: None,
            used_theme_chat_id: None,
            craft_probability_per_mille: None,
            value_amount: None,
            value_currency: None,
            value_usd_amount: None,
            original_details: None,
            resale_parameters: None,
        }
    }

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

    /// Returns `true` if the gift has a known owner and takes purchase offers.
    pub fn can_send_purchase_offer(&self) -> bool {
        self.owner.is_some() && self.minimum_offer_star_count.is_some()
    }

    /// Returns the gift's public `t.me/nft/<name>` link.
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
