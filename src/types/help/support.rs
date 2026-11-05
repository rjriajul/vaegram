use crate::types::User;

/// Telegram's support contact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Support {
    /// Phone number of the support account.
    pub phone_number: String,
    /// The support account to message.
    pub user: User,
}
