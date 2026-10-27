use crate::raw::{base, types};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestedPostPrice {
    Stars(i64),
    Ton(i64),
}

impl SuggestedPostPrice {
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
