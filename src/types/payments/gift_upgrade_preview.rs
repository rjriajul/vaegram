use crate::raw::base;
use crate::types::GiftUpgradeVariants;

/// What upgrading a gift can give, and what it costs now and later.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GiftUpgradePreview {
    /// The models, symbols and backdrops the upgrade can pick from.
    pub variants: GiftUpgradeVariants,
    /// Upgrade prices. Each entry applies from its `date`.
    pub prices: Vec<GiftUpgradePrice>,
    /// Upgrade prices Telegram lists as coming next.
    pub next_prices: Vec<GiftUpgradePrice>,
}

/// The upgrade price from `date` on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct GiftUpgradePrice {
    /// Unix time the price starts to apply.
    pub date: i32,
    /// The price in Stars.
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
