/// What a phone number code is for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum PhoneNumberCodeType {
    /// Logging in.
    #[default]
    Authentication,
    /// Changing the account's phone number.
    Change,
    /// Proving the account owns a number, for services such as Telegram Passport.
    Verify,
}
