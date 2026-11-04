/// Where an offer to buy someone's upgraded gift stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GiftPurchaseOfferState {
    Pending,
    Accepted,
    Rejected,
}
