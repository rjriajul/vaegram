use crate::raw::{base, types};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabeledPrice {
    pub label: String,
    pub amount: i64,
}

impl LabeledPrice {
    pub fn new(label: impl Into<String>, amount: i64) -> LabeledPrice {
        LabeledPrice { label: label.into(), amount }
    }

    pub(crate) fn write(self) -> base::LabeledPrice {
        base::LabeledPrice::LabeledPrice(Box::new(types::LabeledPrice { label: self.label, amount: self.amount }))
    }
}
