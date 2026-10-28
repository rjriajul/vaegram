use crate::raw::types;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invoice {
    pub title: String,
    pub description: String,
    pub start_parameter: String,
    pub currency: String,
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
