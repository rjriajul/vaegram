use crate::raw::types;

/// Telegram Stars gifted by the message's sender.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GiftedStars {
    /// Currency the gift was paid in.
    pub currency: String,
    /// Amount paid, in the smallest units of `currency`.
    pub amount: i64,
    /// Cryptocurrency the gift was paid in. `None` unless it was paid in crypto.
    pub cryptocurrency: Option<String>,
    /// Amount paid in `cryptocurrency`, in its smallest units. `None` unless it was paid in crypto.
    pub cryptocurrency_amount: Option<i64>,
    /// How many Stars the gift contains.
    pub star_count: i64,
    /// Id of the payment transaction. `None` if Telegram reports none.
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
