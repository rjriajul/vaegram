use crate::raw::base;

/// An amount in Telegram Stars (`nanostar_amount` holds 1/10^9 stars), or in nanotons when `ton` is set.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StarAmount {
    /// Whole stars, or nanotons when `ton` is `true`.
    pub amount: i64,
    /// Fractional part in billionths of a star. Always 0 when `ton` is `true`.
    pub nanostar_amount: i32,
    /// `true` if `amount` is in nanotons instead of stars.
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
