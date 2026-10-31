use crate::raw::base;

/// What an upgraded gift is worth; prices are in the smallest units of `currency`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradedGiftValueInfo {
    pub currency: String,
    pub value: i64,
    pub is_value_average: bool,
    pub initial_sale_date: i32,
    pub initial_sale_star_count: i64,
    pub initial_sale_price: i64,
    pub last_sale_date: Option<i32>,
    pub last_sale_price: Option<i64>,
    pub is_last_sale_on_fragment: bool,
    pub minimum_price: Option<i64>,
    pub average_sale_price: Option<i64>,
    pub telegram_listed_gift_count: Option<i32>,
    pub fragment_listed_gift_count: Option<i32>,
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
