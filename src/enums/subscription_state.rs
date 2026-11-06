/// What happened to a user's Star subscription to a bot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SubscriptionState {
    /// The user canceled the subscription.
    Canceled,
    /// The subscription is active again after being canceled.
    Active,
    /// The renewal payment failed.
    Failed,
}
