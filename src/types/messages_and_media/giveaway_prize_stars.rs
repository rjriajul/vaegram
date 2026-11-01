use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::Chat;

/// Telegram Stars won in a giveaway.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiveawayPrizeStars {
    pub star_count: i64,
    pub transaction_id: String,
    /// The chat that ran the giveaway.
    pub boosted_chat: Chat,
    pub giveaway_message_id: i32,
    /// A prize nobody claimed, returned to the chat.
    pub is_unclaimed: bool,
}

impl GiveawayPrizeStars {
    pub(crate) fn parse(raw: &types::MessageActionPrizeStars, entities: &Entities) -> GiveawayPrizeStars {
        GiveawayPrizeStars {
            star_count: raw.stars,
            transaction_id: raw.transaction_id.clone(),
            boosted_chat: Chat::parse(&raw.boost_peer, entities, true),
            giveaway_message_id: raw.giveaway_msg_id,
            is_unclaimed: raw.unclaimed,
        }
    }
}
