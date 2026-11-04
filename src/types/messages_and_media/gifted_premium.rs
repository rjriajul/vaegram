use super::text_with_entities;
use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::MessageEntity;

/// Telegram Premium gifted by the message's sender.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiftedPremium {
    pub currency: String,
    pub amount: i64,
    pub cryptocurrency: Option<String>,
    pub cryptocurrency_amount: Option<i64>,
    pub day_count: i32,
    pub text: Option<String>,
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
