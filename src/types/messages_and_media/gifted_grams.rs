use crate::raw::types;

/// Toncoin gifted by the message's sender.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiftedGrams {
    /// In nanotons.
    pub ton_amount: i64,
    pub transaction_id: Option<String>,
}

impl GiftedGrams {
    pub(crate) fn parse(raw: &types::MessageActionGiftTon) -> GiftedGrams {
        GiftedGrams { ton_amount: raw.crypto_amount, transaction_id: raw.transaction_id.clone() }
    }
}
