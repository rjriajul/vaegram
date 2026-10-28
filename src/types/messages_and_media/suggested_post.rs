use crate::enums::{SuggestedPostRefundReason, SuggestedPostState};
use crate::raw::{base, types};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestedPostPrice {
    Stars(i64),
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
            SuggestedPostPrice::Ton(amount) => {
                base::StarsAmount::StarsTonAmount(Box::new(types::StarsTonAmount { amount }))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SuggestedPostParameters {
    pub price: Option<SuggestedPostPrice>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuggestedPostInfo {
    pub state: SuggestedPostState,
    pub price: Option<SuggestedPostPrice>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuggestedPostApproved {
    pub price: Option<SuggestedPostPrice>,
    pub send_date: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuggestedPostApprovalFailed {
    pub price: Option<SuggestedPostPrice>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestedPostDeclined {
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuggestedPostPaid {
    pub price: SuggestedPostPrice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuggestedPostRefunded {
    pub reason: SuggestedPostRefundReason,
}
