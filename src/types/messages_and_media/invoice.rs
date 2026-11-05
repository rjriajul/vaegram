use crate::raw::types;

/// An invoice sent in a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invoice {
    /// Product name.
    pub title: String,
    /// Product description.
    pub description: String,
    /// Deep-linking parameter that starts the bot to pay this invoice.
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
