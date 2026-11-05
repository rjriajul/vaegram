use crate::raw::base;

/// A Telegram Stars pack the account can buy, priced in `currency`'s smallest units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StarPaymentOption {
    /// Stars in the pack.
    pub star_count: i64,
    /// Currency code of `amount`.
    pub currency: String,
    /// The price in `currency`'s smallest units.
    pub amount: i64,
    /// The in-app store product id. `None` if the pack has none.
    pub store_product_id: Option<String>,
    /// Whether Telegram marks the pack as an extended option.
    pub is_additional: bool,
}

impl StarPaymentOption {
    pub(crate) fn parse(option: base::StarsTopupOption) -> StarPaymentOption {
        let base::StarsTopupOption::StarsTopupOption(o) = option;
        let o = *o;
        StarPaymentOption {
            star_count: o.stars,
            currency: o.currency,
            amount: o.amount,
            store_product_id: o.store_product,
            is_additional: o.extended,
        }
    }
}
