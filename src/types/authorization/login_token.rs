use crate::types::User;

/// Result of a QR code login attempt.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum LoginToken {
    /// Login isn't finished yet.
    #[non_exhaustive]
    Url {
        /// `tg://login` link to show as a QR code for an already logged-in app to scan.
        url: String,
        /// Unix time when the link stops working.
        expires: i32,
    },
    /// An app confirmed the login. Holds the logged-in user.
    Authorized(Box<User>),
}
