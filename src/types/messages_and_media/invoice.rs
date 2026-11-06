use crate::raw::types;

/// An invoice sent in a message.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Invoice {
    /// Name of the product.
    pub title: String,
    /// Description of the product.
    pub description: String,
    /// Bot deep-linking parameter that generates this invoice.
    pub start_parameter: String,
    /// Currency code of the price.
    pub currency: String,
    /// Total price, in the smallest units of `currency`.
    pub total_amount: i64,
}

impl Invoice {
    pub(crate) fn parse(raw: &types::MessageMediaInvoice) -> Invoice {
        Invoice {
            title: raw.title.clone(),
            description: raw.description.clone(),
            start_parameter: raw.start_param.clone(),
            currency: raw.currency.clone(),
            total_amount: raw.total_amount,
        }
    }
}
