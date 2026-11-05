use super::text_with_entities;
use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::MessageEntity;

/// Telegram Premium gifted by the message's sender.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiftedPremium {
    /// Currency the gift was paid in.
    pub currency: String,
    /// Amount paid, in the smallest units of `currency`.
    pub amount: i64,
    /// Cryptocurrency the gift was paid in. `None` unless it was paid in crypto.
    pub cryptocurrency: Option<String>,
    /// Amount paid in `cryptocurrency`, in its smallest units. `None` unless it was paid in crypto.
    pub cryptocurrency_amount: Option<i64>,
    /// How many days of Premium the gift grants.
    pub day_count: i32,
    /// The sender's note. `None` if they didn't add one.
    pub text: Option<String>,
    /// Formatting of `text`. `None` if it has none.
    pub entities: Option<Vec<MessageEntity>>,
}

impl GiftedPremium {
    pub(crate) fn parse(raw: &types::MessageActionGiftPremium, entities: &Entities) -> GiftedPremium {
        let (text, text_entities) = match &raw.message {
            Some(m) => {
                let (text, e) = text_with_entities(m, entities);
                (Some(text), e)
            }
            None => (None, None),
        };
        GiftedPremium {
            currency: raw.currency.clone(),
            amount: raw.amount,
            cryptocurrency: raw.crypto_currency.clone(),
            cryptocurrency_amount: raw.crypto_amount,
            day_count: raw.days,
            text,
            entities: text_entities,
        }
    }
}
