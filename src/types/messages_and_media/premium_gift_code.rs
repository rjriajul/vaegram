use super::text_with_entities;
use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::{Chat, MessageEntity};

/// A Telegram Premium gift code, given directly or won in a giveaway.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PremiumGiftCode {
    /// The chat that created the code, if any.
    pub creator: Option<Chat>,
    /// The message the sender attached.
    pub text: Option<String>,
    /// Formatting entities of `text`.
    pub entities: Option<Vec<MessageEntity>>,
    /// Whether the code was won in a giveaway.
    pub is_from_giveaway: bool,
    /// A giveaway prize nobody claimed.
    pub is_unclaimed: bool,
    /// Currency the code was paid for in, if any.
    pub currency: Option<String>,
    /// Price in the smallest units of `currency`.
    pub amount: Option<i64>,
    /// Cryptocurrency the code was paid for in, if any.
    pub cryptocurrency: Option<String>,
    /// Price in the smallest units of `cryptocurrency`.
    pub cryptocurrency_amount: Option<i64>,
    /// How many days of Premium the code gives.
    pub day_count: i32,
    /// The code's slug, for `t.me/giftcode/<code>`.
    pub code: String,
}

impl PremiumGiftCode {
    pub(crate) fn parse(raw: &types::MessageActionGiftCode, entities: &Entities) -> PremiumGiftCode {
        let (text, text_entities) = match &raw.message {
            Some(m) => {
                let (text, e) = text_with_entities(m, entities);
                (Some(text), e)
            }
            None => (None, None),
        };
        PremiumGiftCode {
            creator: raw.boost_peer.as_ref().map(|p| Chat::parse(p, entities, true)),
            text,
            entities: text_entities,
            is_from_giveaway: raw.via_giveaway,
            is_unclaimed: raw.unclaimed,
            currency: raw.currency.clone(),
            amount: raw.amount,
            cryptocurrency: raw.crypto_currency.clone(),
            cryptocurrency_amount: raw.crypto_amount,
            day_count: raw.days,
            code: raw.slug.clone(),
        }
    }
}
