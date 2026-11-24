use crate::Client;
use crate::raw::types;
use crate::types::StarAmount;

/// This account's Telegram Stars balance changed.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct StarsBalanceUpdated {
    /// The balance now.
    pub balance: StarAmount,
}

impl StarsBalanceUpdated {
    pub(crate) fn parse(_client: &Client, raw: &types::UpdateStarsBalance) -> StarsBalanceUpdated {
        StarsBalanceUpdated { balance: StarAmount::parse(&raw.balance) }
    }
}
