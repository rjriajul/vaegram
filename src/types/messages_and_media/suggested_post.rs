use crate::enums::{SuggestedPostRefundReason, SuggestedPostState};
use crate::raw::{base, types};

/// Price offered for a suggested post.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestedPostPrice {
    /// An amount of Telegram Stars.
    Stars(i64),
    /// An amount of nanotons.
    Ton(i64),
}

impl SuggestedPostPrice {
    pub(crate) fn parse(raw: &base::StarsAmount) -> Self {
        match raw {
            base::StarsAmount::StarsAmount(a) => SuggestedPostPrice::Stars(a.amount),
            base::StarsAmount::StarsTonAmount(a) => SuggestedPostPrice::Ton(a.amount),
        }
    }

    pub(crate) fn write(self) -> base::StarsAmount {
        match self {
            SuggestedPostPrice::Stars(amount) => {
                base::StarsAmount::StarsAmount(Box::new(types::StarsAmount { amount, nanos: 0 }))
            }
            SuggestedPostPrice::Ton(amount) => base::StarsAmount::StarsTonAmount(types::StarsTonAmount { amount }),
        }
    }
}

/// Terms attached when suggesting a post to a channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SuggestedPostParameters {
    /// Price offered for publishing. `None` for a free suggestion.
    pub price: Option<SuggestedPostPrice>,
    /// Unix time to publish at. `None` leaves the time to the channel.
    pub send_date: Option<i32>,
}

impl SuggestedPostParameters {
    pub(crate) fn write(self) -> base::SuggestedPost {
        base::SuggestedPost::SuggestedPost(Box::new(types::SuggestedPost {
            accepted: false,
            rejected: false,
            price: self.price.map(SuggestedPostPrice::write),
            schedule_date: self.send_date,
        }))
    }
}

/// Terms and status of a suggested post.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuggestedPostInfo {
    /// Whether the suggestion is still pending, approved or declined.
    pub state: SuggestedPostState,
    /// Price offered. `None` for a free suggestion.
    pub price: Option<SuggestedPostPrice>,
    /// Unix time the post is set to be published. `None` if no time was set.
    pub send_date: Option<i32>,
}

impl SuggestedPostInfo {
    pub(crate) fn parse(raw: &base::SuggestedPost) -> Self {
        let base::SuggestedPost::SuggestedPost(p) = raw;
        let state = if p.accepted {
            SuggestedPostState::Approved
        } else if p.rejected {
            SuggestedPostState::Declined
        } else {
            SuggestedPostState::Pending
        };
        SuggestedPostInfo { state, price: p.price.as_ref().map(SuggestedPostPrice::parse), send_date: p.schedule_date }
    }
}

/// A suggested post was approved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuggestedPostApproved {
    /// Price paid. `None` for a free suggestion.
    pub price: Option<SuggestedPostPrice>,
    /// Unix time the post is set to be published. `None` if no time was set.
    pub send_date: Option<i32>,
}

/// Approving a suggested post failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuggestedPostApprovalFailed {
    /// Price of the suggestion. `None` for a free one.
    pub price: Option<SuggestedPostPrice>,
}

/// A suggested post was declined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestedPostDeclined {
    /// Reason the admin gave. `None` if there's none.
    pub comment: Option<String>,
}

/// Payment for a suggested post was made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuggestedPostPaid {
    /// The amount paid.
    pub price: SuggestedPostPrice,
}

/// Payment for a suggested post was refunded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuggestedPostRefunded {
    /// Why the payment was refunded.
    pub reason: SuggestedPostRefundReason,
}
