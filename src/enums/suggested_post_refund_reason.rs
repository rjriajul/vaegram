#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuggestedPostRefundReason {
    PostDeleted,
    PaymentRefunded,
}
