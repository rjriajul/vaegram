/// A gift sold through rounds of an auction; see [`Client::get_gift_auction_state`](crate::Client::get_gift_auction_state).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiftAuction {
    /// Slug that identifies the auction.
    pub slug: String,
    /// How many gifts are sold in each round.
    pub gifts_per_round: i32,
    /// Unix time the auction starts.
    pub start_date: i32,
}
