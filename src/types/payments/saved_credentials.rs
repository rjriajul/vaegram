/// A card the provider remembers for this account.
///
/// Pay with it through [`InputCredentials::saved`](crate::types::InputCredentials::saved).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedCredentials {
    /// Id to pass as `saved_credentials_id` to `InputCredentials::saved`.
    pub id: String,
    /// The card's display name.
    pub title: String,
}
