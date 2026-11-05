use crate::types::UniqueGiftInfo;

/// Outcome of crafting a unique gift from other gifts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CraftGiftResult {
    /// The roll succeeded. Holds the new gift.
    Crafted(Box<UniqueGiftInfo>),
    /// The roll failed; the input gifts are gone all the same.
    Failed,
}
