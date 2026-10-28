use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::{StarAmount, TransactionPartner};

#[derive(Debug, Clone, PartialEq)]
pub struct StarTransaction {
    pub id: String,
    pub amount: StarAmount,
    pub date: i32,
    pub refund: bool,
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

#[derive(Debug, Clone, PartialEq)]
pub struct StarTransactions {
    pub transactions: Vec<StarTransaction>,
    pub next_offset: Option<String>,
}
