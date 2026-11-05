/// Where an offer to buy someone's upgraded gift stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GiftPurchaseOfferState {
    /// Neither accepted nor declined yet.
    Pending,
    /// The gift's owner accepted the offer.
    Accepted,
    /// The gift's owner declined the offer.
    Rejected,
}
