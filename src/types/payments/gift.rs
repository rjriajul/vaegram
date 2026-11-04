use crate::dispatcher::Entities;
use crate::raw::{base, types};
use crate::types::{Chat, Media, Sticker};

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniqueGiftAttribute {
    pub name: String,
    pub sticker: Sticker,
    pub rarity_per_mille: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniqueGiftBackdrop {
    pub name: String,
    pub center_color: i32,
    pub edge_color: i32,
    pub symbol_color: i32,
    pub text_color: i32,
    pub rarity_per_mille: Option<i32>,
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
        })
    }
}

impl UniqueGift {
    pub(crate) fn parse(raw: &types::StarGiftUnique, entities: &Entities) -> UniqueGift {
        let (mut model, mut symbol, mut backdrop) = (None, None, None);
        for raw_attribute in &raw.attributes {
            match raw_attribute {
                base::StarGiftAttribute::StarGiftAttributeModel(a) => {
                    model = attribute(&a.name, &a.document, &a.rarity)
                }
                base::StarGiftAttribute::StarGiftAttributePattern(a) => {
                    symbol = attribute(&a.name, &a.document, &a.rarity)
                }
                base::StarGiftAttribute::StarGiftAttributeBackdrop(a) => backdrop = Some(self::backdrop(a)),
                _ => {}
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
        }
    }
}

pub(super) fn attribute(
    name: &str,
    document: &base::Document,
    rarity: &base::StarGiftAttributeRarity,
) -> Option<UniqueGiftAttribute> {
    Some(UniqueGiftAttribute { name: name.to_owned(), sticker: sticker(document)?, rarity_per_mille: permille(rarity) })
}

pub(super) fn backdrop(raw: &types::StarGiftAttributeBackdrop) -> UniqueGiftBackdrop {
    UniqueGiftBackdrop {
        name: raw.name.clone(),
        center_color: raw.center_color,
        edge_color: raw.edge_color,
        symbol_color: raw.pattern_color,
        text_color: raw.text_color,
        rarity_per_mille: permille(&raw.rarity),
    }
}

pub(super) fn sticker(document: &base::Document) -> Option<Sticker> {
    match Media::from_document(document)? {
        Media::Sticker(sticker) => Some(sticker),
        _ => None,
    }
}

fn permille(rarity: &base::StarGiftAttributeRarity) -> Option<i32> {
    match rarity {
        base::StarGiftAttributeRarity::StarGiftAttributeRarity(r) => Some(r.permille),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::UniqueGift;
    use crate::dispatcher::Entities;
    use crate::raw::types;

    #[test]
    fn unique_gifts_keep_their_own_id() {
        let raw = types::StarGiftUnique {
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
        let gift = UniqueGift::parse(&raw, &Entities::default());
        assert_eq!((gift.id, gift.gift_id, gift.number), (1, 2, 3));
    }
}
