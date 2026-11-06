use crate::raw::{base, types};
use crate::types::LabeledPrice;

/// One way to ship an order, with its price lines.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ShippingOption {
    /// Identifier the bot picks. Comes back as [`crate::types::PreCheckoutQuery::shipping_option_id`].
    pub id: String,
    /// Name shown to the buyer.
    pub title: String,
    /// Price lines that make up the cost of this option.
    pub prices: Vec<LabeledPrice>,
}

impl ShippingOption {
    /// Creates a shipping option.
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        prices: impl IntoIterator<Item = LabeledPrice>,
    ) -> ShippingOption {
        ShippingOption { id: id.into(), title: title.into(), prices: prices.into_iter().collect() }
    }

    pub(crate) fn parse(raw: &base::ShippingOption) -> ShippingOption {
        let base::ShippingOption::ShippingOption(o) = raw;
        let prices = o
            .prices
            .iter()
            .map(|base::LabeledPrice::LabeledPrice(p)| LabeledPrice::new(p.label.clone(), p.amount))
            .collect();
        ShippingOption { id: o.id.clone(), title: o.title.clone(), prices }
    }

    pub(crate) fn write(self) -> base::ShippingOption {
        let prices = self.prices.into_iter().map(LabeledPrice::write).collect();
        base::ShippingOption::ShippingOption(Box::new(types::ShippingOption { id: self.id, title: self.title, prices }))
    }
}
