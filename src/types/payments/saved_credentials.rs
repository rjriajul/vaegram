/// A card the provider remembers for this account. Pay with it through [`InputCredentials::saved`](crate::types::InputCredentials::saved).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedCredentials {
    pub id: String,
    pub title: String,
}
