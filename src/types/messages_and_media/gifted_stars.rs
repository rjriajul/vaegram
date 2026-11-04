use crate::raw::types;

/// Telegram Stars gifted by the message's sender.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiftedStars {
    pub currency: String,
    pub amount: i64,
    pub cryptocurrency: Option<String>,
    pub cryptocurrency_amount: Option<i64>,
    pub star_count: i64,
    pub transaction_id: Option<String>,
}

impl GiftedStars {
    pub(crate) fn parse(raw: &types::MessageActionGiftStars) -> GiftedStars {
        GiftedStars {
            currency: raw.currency.clone(),
            amount: raw.amount,
            cryptocurrency: raw.crypto_currency.clone(),
            cryptocurrency_amount: raw.crypto_amount,
            star_count: raw.stars,
            transaction_id: raw.transaction_id.clone(),
        }
    }
}
