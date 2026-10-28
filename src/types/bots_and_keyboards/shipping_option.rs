use crate::raw::{base, types};
use crate::types::LabeledPrice;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShippingOption {
    pub id: String,
    pub title: String,
    pub prices: Vec<LabeledPrice>,
}

impl ShippingOption {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        prices: impl IntoIterator<Item = LabeledPrice>,
    ) -> ShippingOption {
        ShippingOption { id: id.into(), title: title.into(), prices: prices.into_iter().collect() }
    }

    pub(crate) fn write(self) -> base::ShippingOption {
        let prices = self.prices.into_iter().map(LabeledPrice::write).collect();
        base::ShippingOption::ShippingOption(Box::new(types::ShippingOption { id: self.id, title: self.title, prices }))
    }
}
