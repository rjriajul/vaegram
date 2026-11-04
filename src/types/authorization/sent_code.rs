use crate::enums::{NextCodeType, SentCodeType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentCode {
    pub kind: SentCodeType,
    pub phone_code_hash: String,
    pub next_kind: Option<NextCodeType>,
    pub timeout: Option<i32>,
}
