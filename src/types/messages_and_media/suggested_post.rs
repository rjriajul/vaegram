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
    /// A free suggestion with the publishing time left to the channel, like `Default`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Offers this price for publishing.
    #[must_use]
    pub fn price(mut self, price: SuggestedPostPrice) -> Self {
        self.price = Some(price);
        self
    }

    /// Asks for the post to be published at this Unix time.
    #[must_use]
    pub fn send_date(mut self, send_date: i32) -> Self {
        self.send_date = Some(send_date);
        self
    }

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
    /// Reason the admin gave. `None` if the admin gave none.
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

#[cfg(test)]
mod tests {
    use super::{SuggestedPostParameters, SuggestedPostPrice};
    use crate::raw::base;

    #[test]
    fn builds_from_new_and_setters() {
        let base::SuggestedPost::SuggestedPost(free) = SuggestedPostParameters::new().write();
        assert!(free.price.is_none() && free.schedule_date.is_none() && !free.accepted && !free.rejected);
        let paid = SuggestedPostParameters::new().price(SuggestedPostPrice::Stars(50)).send_date(1_700_000_000);
        let base::SuggestedPost::SuggestedPost(raw) = paid.write();
        assert_eq!(raw.schedule_date, Some(1_700_000_000));
        assert!(matches!(raw.price, Some(base::StarsAmount::StarsAmount(a)) if a.amount == 50));
    }
}
