use crate::raw::base;

/// A Premium gift-code package that can be bought, priced in `currency`'s smallest units.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct PremiumGiftCodeOption {
    /// How many gift codes, or giveaway winners, the package covers.
    pub user_count: i32,
    /// Months of Premium each code gives.
    pub months: i32,
    /// Three-letter ISO 4217 currency code of `amount`.
    pub currency: String,
    /// Price of the whole package in the smallest unit of `currency`.
    pub amount: i64,
    /// The in-app store product id. `None` if the package has none.
    pub store_product_id: Option<String>,
    /// How many units of the store product make up the package. `None` if the package has no store product.
    pub store_quantity: Option<i32>,
}

impl PremiumGiftCodeOption {
    pub(crate) fn parse(option: base::PremiumGiftCodeOption) -> PremiumGiftCodeOption {
        let base::PremiumGiftCodeOption::PremiumGiftCodeOption(o) = option;
        let o = *o;
        PremiumGiftCodeOption {
            user_count: o.users,
            months: o.months,
            currency: o.currency,
            amount: o.amount,
            store_product_id: o.store_product,
            store_quantity: o.store_quantity,
        }
    }
}
