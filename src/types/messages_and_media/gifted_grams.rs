use crate::raw::types;

/// Toncoin gifted by the message's sender.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GiftedGrams {
    /// Amount of Toncoin, in nanotons.
    pub ton_amount: i64,
    /// Id of the blockchain transaction. `None` if Telegram sent none.
    pub transaction_id: Option<String>,
}

impl GiftedGrams {
    pub(crate) fn parse(raw: &types::MessageActionGiftTon) -> GiftedGrams {
        GiftedGrams { ton_amount: raw.crypto_amount, transaction_id: raw.transaction_id.clone() }
    }
}
