use crate::raw::base;

/// An amount in Telegram Stars (`nanostar_amount` holds 1/10^9 stars), or in nanotons when `ton` is set.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct StarAmount {
    /// Whole stars, or nanotons when `ton` is `true`.
    pub amount: i64,
    /// Fractional part in billionths of a star. Always 0 when `ton` is `true`.
    pub nanostar_amount: i32,
    /// `true` if `amount` is in nanotons instead of stars.
    pub ton: bool,
}

impl StarAmount {
    /// Creates an amount of `amount` whole Stars; [`ton`](Self::ton) makes it nanotons instead.
    pub fn new(amount: i64) -> Self {
        StarAmount { amount, ..StarAmount::default() }
    }

    /// Adds a fractional part, in billionths of a Star. Leave it at 0 for nanotons.
    #[must_use]
    pub fn nanostar_amount(mut self, nanostar_amount: i32) -> Self {
        self.nanostar_amount = nanostar_amount;
        self
    }

    /// Sets whether `amount` counts nanotons instead of Stars.
    #[must_use]
    pub fn ton(mut self, ton: bool) -> Self {
        self.ton = ton;
        self
    }

    pub(crate) fn parse(raw: &base::StarsAmount) -> StarAmount {
        match raw {
            base::StarsAmount::StarsAmount(a) => StarAmount { amount: a.amount, nanostar_amount: a.nanos, ton: false },
            base::StarsAmount::StarsTonAmount(a) => StarAmount { amount: a.amount, nanostar_amount: 0, ton: true },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StarAmount;
    use crate::raw::{base, types};

    #[test]
    fn new_counts_whole_stars_until_set_otherwise() {
        let stars = base::StarsAmount::StarsAmount(Box::new(types::StarsAmount { amount: 3, nanos: 0 }));
        assert_eq!(StarAmount::new(3), StarAmount::parse(&stars));
        let fraction = base::StarsAmount::StarsAmount(Box::new(types::StarsAmount { amount: 3, nanos: 7 }));
        assert_eq!(StarAmount::new(3).nanostar_amount(7), StarAmount::parse(&fraction));
        let ton = base::StarsAmount::StarsTonAmount(types::StarsTonAmount { amount: 5 });
        assert_eq!(StarAmount::new(5).ton(true), StarAmount::parse(&ton));
    }
}
