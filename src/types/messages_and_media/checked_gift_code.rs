use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::{Chat, User};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedGiftCode {
    pub date: i32,
    pub days: i32,
    pub via_giveaway: bool,
    pub from_chat: Option<Chat>,
    pub giveaway_message_id: Option<i32>,
    /// `None` for an unclaimed giveaway prize: anyone holding the link can apply it.
    pub to_user: Option<User>,
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
