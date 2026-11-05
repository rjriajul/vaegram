use crate::raw::{base, types};

/// One line item of an invoice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabeledPrice {
    /// What the line is for, shown to the buyer.
    pub label: String,
    /// Price in the smallest unit of the invoice currency.
    pub amount: i64,
}

impl LabeledPrice {
    /// Creates a price line from a label and an amount.
    pub fn new(label: impl Into<String>, amount: i64) -> LabeledPrice {
        LabeledPrice { label: label.into(), amount }
    }

    pub(crate) fn write(self) -> base::LabeledPrice {
        base::LabeledPrice::LabeledPrice(Box::new(types::LabeledPrice { label: self.label, amount: self.amount }))
    }
}
