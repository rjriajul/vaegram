use crate::raw::base;
use crate::types::GiftUpgradeVariants;

/// What upgrading a gift can give, and what it costs now and later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiftUpgradePreview {
    pub variants: GiftUpgradeVariants,
    pub prices: Vec<GiftUpgradePrice>,
    pub next_prices: Vec<GiftUpgradePrice>,
}

/// The upgrade price from `date` on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GiftUpgradePrice {
    pub date: i32,
    pub star_count: i64,
}

impl GiftUpgradePreview {
    pub(crate) fn parse(raw: &base::payments::StarGiftUpgradePreview) -> GiftUpgradePreview {
        let base::payments::StarGiftUpgradePreview::StarGiftUpgradePreview(raw) = raw;
        let prices = |prices: &[base::StarGiftUpgradePrice]| -> Vec<GiftUpgradePrice> {
            prices
                .iter()
                .map(|base::StarGiftUpgradePrice::StarGiftUpgradePrice(p)| GiftUpgradePrice {
                    date: p.date,
                    star_count: p.upgrade_stars,
                })
                .collect()
        };
        GiftUpgradePreview {
            variants: GiftUpgradeVariants::parse(&raw.sample_attributes),
            prices: prices(&raw.prices),
            next_prices: prices(&raw.next_prices),
        }
    }
}
