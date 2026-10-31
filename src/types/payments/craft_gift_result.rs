use crate::types::UniqueGiftInfo;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CraftGiftResult {
    Crafted(Box<UniqueGiftInfo>),
    /// The roll failed; the input gifts are gone all the same.
    Failed,
}
