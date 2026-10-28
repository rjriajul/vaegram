#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubscriptionState {
    Canceled,
    Active,
    Failed,
}
