use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, types};
use crate::types::{Chat, Gift, User};

/// Current state of a gift auction and the account's place in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiftAuctionState {
    /// The gift being auctioned.
    pub gift: Gift,
    /// Whether the auction is running or over.
    pub status: GiftAuctionStatus,
    /// The account's bid. `None` if the account has no bid in this auction.
    pub my_bid: Option<GiftAuctionBid>,
    /// How many gifts the account has won in this auction.
    pub acquired_gift_count: i32,
    /// `true` if Telegram returned the account's bid.
    pub is_bid_returned: bool,
    /// Timeout Telegram attaches to the state.
    pub timeout: i32,
}

/// Whether a gift auction is running or over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GiftAuctionStatus {
    /// The auction is running.
    Active(Box<ActiveGiftAuction>),
    /// The auction is over.
    Finished(FinishedGiftAuction),
}

/// Details of a running gift auction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveGiftAuction {
    /// Unix time the auction started.
    pub start_date: i32,
    /// Unix time the auction ends.
    pub end_date: i32,
    /// Smallest bid in Stars Telegram accepts right now.
    pub min_bid_star_count: i64,
    /// Bid levels of the auction.
    pub bid_levels: Vec<GiftAuctionBidLevel>,
    /// Users with the highest bids.
    pub top_bidders: Vec<User>,
    /// Unix time the next round starts.
    pub next_round_date: i32,
    /// Serial number of the last gift sold.
    pub last_gift_number: i32,
    /// Number of gifts not sold yet.
    pub remaining_gift_count: i32,
    /// Number of the round in progress.
    pub current_round: i32,
    /// Number of rounds in the whole auction.
    pub total_round_count: i32,
    /// Schedule of the rounds.
    pub rounds: Vec<GiftAuctionRound>,
}

/// Details of a finished gift auction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishedGiftAuction {
    /// Unix time the auction started.
    pub start_date: i32,
    /// Unix time the auction ended.
    pub end_date: i32,
    /// Average price in Stars the gifts sold for.
    pub average_price_star_count: i64,
    /// Number of won gifts now listed for resale. `None` if Telegram doesn't send it.
    pub listed_count: Option<i32>,
    /// Number of won gifts listed on Fragment.
    pub fragment_listed_count: Option<i32>,
    /// Link to the Fragment listings. Set together with `fragment_listed_count`.
    pub fragment_listed_url: Option<String>,
}

/// The account's bid in a gift auction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiftAuctionBid {
    /// Amount bid, in Stars.
    pub star_count: i64,
    /// Unix time the bid was placed.
    pub date: i32,
    /// Smallest amount in Stars the account can bid next.
    pub min_star_count: i64,
    /// Chat that receives the gift if the bid wins.
    pub recipient: Chat,
}

/// A bid level in a gift auction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GiftAuctionBidLevel {
    /// Position of the level in the ranking.
    pub position: i32,
    /// Stars bid at this level.
    pub star_count: i64,
    /// Unix time the level was reached.
    pub date: i32,
}

/// One round of a gift auction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GiftAuctionRound {
    /// Number of the round.
    pub number: i32,
    /// Length of the round in seconds.
    pub duration: i32,
    /// Number of top bids that can extend the round. `None` if the round can't be extended.
    pub extend_top: Option<i32>,
    /// Seconds before the round ends in which a top bid extends it. `None` if the round can't be extended.
    pub extend_window: Option<i32>,
}

impl GiftAuctionState {
    pub(crate) fn parse(raw: &types::payments::StarGiftAuctionState, entities: &Entities) -> Result<GiftAuctionState> {
        let gift = match &raw.gift {
            base::StarGift::StarGift(gift) => Gift::parse(gift, entities)
                .ok_or(Error::Unexpected("payments.getStarGiftAuctionState sent a gift with no sticker"))?,
            base::StarGift::StarGiftUnique(_) => {
                return Err(Error::Unexpected("payments.getStarGiftAuctionState sent an upgraded gift"));
            }
        };
        let status = match &raw.state {
            base::StarGiftAuctionState::StarGiftAuctionState(s) => {
                GiftAuctionStatus::Active(Box::new(ActiveGiftAuction {
                    start_date: s.start_date,
                    end_date: s.end_date,
                    min_bid_star_count: s.min_bid_amount,
                    bid_levels: s
                        .bid_levels
                        .iter()
                        .map(|base::AuctionBidLevel::AuctionBidLevel(l)| GiftAuctionBidLevel {
                            position: l.pos,
                            star_count: l.amount,
                            date: l.date,
                        })
                        .collect(),
                    top_bidders: s.top_bidders.iter().map(|&id| User::parse(id, entities)).collect(),
                    next_round_date: s.next_round_at,
                    last_gift_number: s.last_gift_num,
                    remaining_gift_count: s.gifts_left,
                    current_round: s.current_round,
                    total_round_count: s.total_rounds,
                    rounds: s.rounds.iter().map(round).collect(),
                }))
            }
            base::StarGiftAuctionState::StarGiftAuctionStateFinished(s) => {
                GiftAuctionStatus::Finished(FinishedGiftAuction {
                    start_date: s.start_date,
                    end_date: s.end_date,
                    average_price_star_count: s.average_price,
                    listed_count: s.listed_count,
                    fragment_listed_count: s.fragment_listed_count,
                    fragment_listed_url: s.fragment_listed_url.clone(),
                })
            }
            base::StarGiftAuctionState::StarGiftAuctionStateNotModified => {
                return Err(Error::Unexpected("payments.getStarGiftAuctionState sent no state"));
            }
        };
        let base::StarGiftAuctionUserState::StarGiftAuctionUserState(user) = &raw.user_state;
        let my_bid = match (user.bid_amount, user.bid_date, user.min_bid_amount, &user.bid_peer) {
            (Some(star_count), Some(date), Some(min_star_count), Some(peer)) => {
                Some(GiftAuctionBid { star_count, date, min_star_count, recipient: Chat::parse(peer, entities, false) })
            }
            _ => None,
        };
        Ok(GiftAuctionState {
            gift,
            status,
            my_bid,
            acquired_gift_count: user.acquired_count,
            is_bid_returned: user.returned,
            timeout: raw.timeout,
        })
    }
}

fn round(raw: &base::StarGiftAuctionRound) -> GiftAuctionRound {
    match raw {
        base::StarGiftAuctionRound::StarGiftAuctionRound(r) => {
            GiftAuctionRound { number: r.num, duration: r.duration, extend_top: None, extend_window: None }
        }
        base::StarGiftAuctionRound::StarGiftAuctionRoundExtendable(r) => GiftAuctionRound {
            number: r.num,
            duration: r.duration,
            extend_top: Some(r.extend_top),
            extend_window: Some(r.extend_window),
        },
    }
}
