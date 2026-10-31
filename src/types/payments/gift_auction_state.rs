use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, types};
use crate::types::{Chat, Gift, User};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiftAuctionState {
    pub gift: Gift,
    pub status: GiftAuctionStatus,
    /// The user's current bid, if they have one in this auction.
    pub my_bid: Option<GiftAuctionBid>,
    pub acquired_gift_count: i32,
    pub is_bid_returned: bool,
    pub timeout: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GiftAuctionStatus {
    Active(Box<ActiveGiftAuction>),
    Finished(FinishedGiftAuction),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveGiftAuction {
    pub start_date: i32,
    pub end_date: i32,
    pub min_bid_star_count: i64,
    pub bid_levels: Vec<GiftAuctionBidLevel>,
    pub top_bidders: Vec<User>,
    pub next_round_date: i32,
    pub last_gift_number: i32,
    pub remaining_gift_count: i32,
    pub current_round: i32,
    pub total_round_count: i32,
    pub rounds: Vec<GiftAuctionRound>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishedGiftAuction {
    pub start_date: i32,
    pub end_date: i32,
    pub average_price_star_count: i64,
    pub listed_count: Option<i32>,
    pub fragment_listed_count: Option<i32>,
    pub fragment_listed_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiftAuctionBid {
    pub star_count: i64,
    pub date: i32,
    pub min_star_count: i64,
    pub recipient: Chat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GiftAuctionBidLevel {
    pub position: i32,
    pub star_count: i64,
    pub date: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GiftAuctionRound {
    pub number: i32,
    pub duration: i32,
    pub extend_top: Option<i32>,
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
