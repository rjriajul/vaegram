use crate::types::UniqueGiftInfo;

/// Outcome of crafting a unique gift from other gifts.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum CraftGiftResult {
    /// The roll succeeded. Holds the new gift.
    Crafted(Box<UniqueGiftInfo>),
    /// The roll failed. The input gifts are gone all the same.
    Failed,
}
