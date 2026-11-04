/// A gift sold through rounds of an auction; see [`Client::get_gift_auction_state`](crate::Client::get_gift_auction_state).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiftAuction {
    pub slug: String,
    pub gifts_per_round: i32,
    pub start_date: i32,
}
