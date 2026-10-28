use crate::raw::base;

/// An amount in Telegram Stars (`nanostar_amount` holds 1/10^9 stars), or in nanotons when `ton` is set.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StarAmount {
    pub amount: i64,
    pub nanostar_amount: i32,
    pub ton: bool,
}

impl StarAmount {
    pub(crate) fn parse(raw: &base::StarsAmount) -> StarAmount {
        match raw {
            base::StarsAmount::StarsAmount(a) => StarAmount { amount: a.amount, nanostar_amount: a.nanos, ton: false },
            base::StarsAmount::StarsTonAmount(a) => StarAmount { amount: a.amount, nanostar_amount: 0, ton: true },
        }
    }
}
