use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::{Chat, User};

/// What Telegram reports about a Premium gift code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedGiftCode {
    /// When the code was created, as a Unix timestamp.
    pub date: i32,
    /// Days of Premium the code grants.
    pub days: i32,
    /// `true` if the code was a giveaway prize.
    pub via_giveaway: bool,
    /// The chat that issued the code. `None` if Telegram sent no sender.
    pub from_chat: Option<Chat>,
    /// Id of the giveaway message. `None` if the code isn't from a giveaway.
    pub giveaway_message_id: Option<i32>,
    /// The user the code was given to. `None` if Telegram names no recipient.
    pub to_user: Option<User>,
    /// When the code was applied, as a Unix timestamp. `None` if it is unused.
    pub used_date: Option<i32>,
}

impl CheckedGiftCode {
    pub(crate) fn parse(raw: &types::payments::CheckedGiftCode, entities: &Entities) -> CheckedGiftCode {
        CheckedGiftCode {
            date: raw.date,
            days: raw.days,
            via_giveaway: raw.via_giveaway,
            from_chat: raw.from_id.as_ref().map(|p| Chat::parse(p, entities, true)),
            giveaway_message_id: raw.giveaway_msg_id,
            to_user: raw.to_id.map(|id| User::parse(id, entities)),
            used_date: raw.used_date,
        }
    }
}
