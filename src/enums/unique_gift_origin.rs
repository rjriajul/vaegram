#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum UniqueGiftOrigin {
    Upgrade,
    GiftedUpgrade,
    Transfer,
    Resale,
    Offer,
    Craft,
}
