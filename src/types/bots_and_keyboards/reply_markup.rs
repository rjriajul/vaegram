use crate::raw::base;
use crate::types::{ForceReply, InlineKeyboardMarkup, ReplyKeyboardMarkup, ReplyKeyboardRemove};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplyMarkup {
    Inline(InlineKeyboardMarkup),
    Keyboard(ReplyKeyboardMarkup),
    Remove(ReplyKeyboardRemove),
    ForceReply(ForceReply),
}

impl ReplyMarkup {
    pub(crate) fn write(&self) -> base::ReplyMarkup {
        match self {
            ReplyMarkup::Inline(m) => m.write(),
            ReplyMarkup::Keyboard(m) => m.write(),
            ReplyMarkup::Remove(m) => m.write(),
            ReplyMarkup::ForceReply(m) => m.write(),
        }
    }
}

impl From<InlineKeyboardMarkup> for ReplyMarkup {
    fn from(m: InlineKeyboardMarkup) -> Self {
        ReplyMarkup::Inline(m)
    }
}

impl From<ReplyKeyboardMarkup> for ReplyMarkup {
    fn from(m: ReplyKeyboardMarkup) -> Self {
        ReplyMarkup::Keyboard(m)
    }
}

impl From<ReplyKeyboardRemove> for ReplyMarkup {
    fn from(m: ReplyKeyboardRemove) -> Self {
        ReplyMarkup::Remove(m)
    }
}

impl From<ForceReply> for ReplyMarkup {
    fn from(m: ForceReply) -> Self {
        ReplyMarkup::ForceReply(m)
    }
}
