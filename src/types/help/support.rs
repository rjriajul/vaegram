use crate::types::User;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Support {
    pub phone_number: String,
    pub user: User,
}
