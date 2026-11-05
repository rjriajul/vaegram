/// How a unique gift reached its current owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum UniqueGiftOrigin {
    /// The owner upgraded a regular gift.
    Upgrade,
    /// The gift was upgraded with an upgrade its sender prepaid.
    GiftedUpgrade,
    /// Another user transferred the gift.
    Transfer,
    /// The owner bought it on the resale market.
    Resale,
    /// The owner got it by accepting a purchase offer.
    Offer,
    /// The owner crafted it.
    Craft,
    /// The gift was assigned from the blockchain.
    Blockchain,
}
