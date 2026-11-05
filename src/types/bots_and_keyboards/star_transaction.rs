use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::{StarAmount, TransactionPartner};

/// One Stars transaction.
#[derive(Debug, Clone, PartialEq)]
pub struct StarTransaction {
    /// Telegram's id for the transaction.
    pub id: String,
    /// Amount moved by the transaction.
    pub amount: StarAmount,
    /// When the transaction happened, as a Unix timestamp.
    pub date: i32,
    /// Set when the transaction is a refund.
    pub refund: bool,
    /// The other party in the transaction.
    pub partner: TransactionPartner,
}

impl StarTransaction {
    pub(crate) fn parse(raw: types::StarsTransaction, entities: &Entities) -> StarTransaction {
        let partner = TransactionPartner::parse(&raw, entities);
        StarTransaction {
            id: raw.id,
            amount: StarAmount::parse(&raw.amount),
            date: raw.date,
            refund: raw.refund,
            partner,
        }
    }
}

/// A page of Stars transactions.
#[derive(Debug, Clone, PartialEq)]
pub struct StarTransactions {
    /// The transactions on this page.
    pub transactions: Vec<StarTransaction>,
    /// Offset that fetches the next page. `None` on the last page.
    pub next_offset: Option<String>,
}
