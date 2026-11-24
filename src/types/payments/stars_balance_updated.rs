use crate::Client;
use crate::raw::types;
use crate::types::StarAmount;

/// This account's Telegram Stars balance changed.
#[non_exhaustive]
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct StarsBalanceUpdated {
    /// The balance now.
    pub balance: StarAmount,
    #[cfg_attr(feature = "serde", serde(skip))]
    client: Client,
}

impl StarsBalanceUpdated {
    pub(crate) fn parse(client: &Client, raw: &types::UpdateStarsBalance) -> StarsBalanceUpdated {
        StarsBalanceUpdated { balance: StarAmount::parse(&raw.balance), client: client.clone() }
    }

    /// The [`Client`] that received this update.
    pub fn client(&self) -> &Client {
        &self.client
    }
}
