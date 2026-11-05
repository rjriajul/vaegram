use crate::raw::base;

/// What an upgraded gift is worth; prices are in the smallest units of `currency`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradedGiftValueInfo {
    /// Currency code of the prices.
    pub currency: String,
    /// Estimated value of the gift.
    pub value: i64,
    /// Whether `value` is an average rather than a single price.
    pub is_value_average: bool,
    /// Unix time of the first sale.
    pub initial_sale_date: i32,
    /// Stars the first sale cost.
    pub initial_sale_star_count: i64,
    /// Price of the first sale.
    pub initial_sale_price: i64,
    /// Unix time of the latest sale. `None` if the gift hasn't been resold.
    pub last_sale_date: Option<i32>,
    /// Price of the latest sale. `None` if the gift hasn't been resold.
    pub last_sale_price: Option<i64>,
    /// Whether the latest sale happened on Fragment.
    pub is_last_sale_on_fragment: bool,
    /// Lowest price among current listings. `None` if Telegram leaves it out.
    pub minimum_price: Option<i64>,
    /// Average sale price, if Telegram sends one.
    pub average_sale_price: Option<i64>,
    /// Number of listings of this gift on Telegram, if Telegram sends it.
    pub telegram_listed_gift_count: Option<i32>,
    /// Number of listings of this gift on Fragment. Set together with `fragment_url`.
    pub fragment_listed_gift_count: Option<i32>,
    /// Link to the Fragment listings. Set together with `fragment_listed_gift_count`.
    pub fragment_url: Option<String>,
}

impl UpgradedGiftValueInfo {
    pub(crate) fn parse(raw: base::payments::UniqueStarGiftValueInfo) -> UpgradedGiftValueInfo {
        let base::payments::UniqueStarGiftValueInfo::UniqueStarGiftValueInfo(raw) = raw;
        UpgradedGiftValueInfo {
            currency: raw.currency,
            value: raw.value,
            is_value_average: raw.value_is_average,
            initial_sale_date: raw.initial_sale_date,
            initial_sale_star_count: raw.initial_sale_stars,
            initial_sale_price: raw.initial_sale_price,
            last_sale_date: raw.last_sale_date,
            last_sale_price: raw.last_sale_price,
            is_last_sale_on_fragment: raw.last_sale_on_fragment,
            minimum_price: raw.floor_price,
            average_sale_price: raw.average_price,
            telegram_listed_gift_count: raw.listed_count,
            fragment_listed_gift_count: raw.fragment_listed_count,
            fragment_url: raw.fragment_listed_url,
        }
    }
}
