use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::Chat;

/// One of your boost slots and the chat it currently boosts.
/// One of your boost slots and the chat it currently boosts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MyBoost {
    /// Slot number.
    pub slot: i32,
    /// The boosted chat. `None` while the slot is unused.
    pub chat: Option<Chat>,
    /// Unix time the boost was applied.
    pub date: i32,
    /// Unix time the boost expires.
    pub expire_date: i32,
    /// Unix time the slot can be moved to another chat. `None` if there is no cooldown.
    pub cooldown_until_date: Option<i32>,
}

impl MyBoost {
    pub(crate) fn parse(raw: &types::MyBoost, entities: &Entities) -> MyBoost {
        MyBoost {
            slot: raw.slot,
            chat: raw.peer.as_ref().map(|p| Chat::parse(p, entities, true)),
            date: raw.date,
            expire_date: raw.expires,
            cooldown_until_date: raw.cooldown_until_date,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MyBoost;
    use crate::dispatcher::Entities;
    use crate::raw::types;

    #[test]
    fn an_unused_slot_has_no_chat() {
        let raw = types::MyBoost { slot: 2, peer: None, date: 5, expires: 9, cooldown_until_date: None };
        let boost = MyBoost::parse(&raw, &Entities::default());
        assert_eq!((boost.slot, boost.chat, boost.expire_date, boost.cooldown_until_date), (2, None, 9, None));
    }
}
